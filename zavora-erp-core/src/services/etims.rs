//! KRA eTIMS OSCU/VSCU integration — real-time tax-invoice transmission.
//!
//! Under KRA's electronic Tax Invoice Management System, a compliant tax invoice
//! must be transmitted to KRA through a Sales Control Unit — either the **OSCU**
//! (Online SCU) or the **VSCU** (Virtual SCU). Both speak the same JSON API; the
//! ERP acts as the VSCU. The lifecycle is:
//!
//!   1. **Initialise the device** (`/selectInitOsdcInfo`) with the taxpayer PIN,
//!      branch id and device serial → KRA returns the SCU id, MRC number and a
//!      communication key (`cmcKey`) used to authenticate later calls.
//!   2. **Transmit each sale** (`/saveTrnsSalesOsdc`) → KRA signs it and returns
//!      the receipt number, internal data and signature that make the printed
//!      receipt a legal tax invoice, plus a QR the buyer can verify.
//!   3. (optional) register items and transmit purchases/stock movements.
//!
//! Credentials and the environment (sandbox/production) are per-entity in
//! `etims_devices`. The base URL can be overridden with `ETIMS_BASE_URL`. Without
//! an initialised, enabled device this module is inert — invoices simply stay
//! `not_transmitted`, exactly as before.

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::types::VatTreatment;
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

/// Default KRA item-classification code used when a product hasn't been
/// classified yet (a generic "other" class). Real deployments classify each item.
const DEFAULT_ITEM_CLS: &str = "5022110800";

// ─── Device config ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct EtimsDeviceRow {
    pub entity_id: Uuid,
    pub enabled: bool,
    pub environment: String,
    pub pin: Option<String>,
    pub bhf_id: String,
    pub dvc_srl_no: Option<String>,
    pub sdc_id: Option<String>,
    pub mrc_no: Option<String>,
    #[serde(skip_serializing)]
    pub cmc_key: Option<String>,
    pub initialized: bool,
    pub initialized_at: Option<chrono::DateTime<Utc>>,
    pub last_invc_no: i64,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl EtimsDeviceRow {
    fn base_url(&self) -> String {
        if let Ok(u) = std::env::var("ETIMS_BASE_URL") {
            return u.trim_end_matches('/').to_string();
        }
        match self.environment.as_str() {
            "production" | "prod" => "https://etims-api.kra.go.ke/etims-api".into(),
            _ => "https://etims-api-sbx.kra.go.ke/etims-api".into(),
        }
    }
    /// The buyer-facing verification host (for the receipt QR).
    fn verify_host(&self) -> &'static str {
        match self.environment.as_str() {
            "production" | "prod" => "https://etims.kra.go.ke",
            _ => "https://etims-sbx.kra.go.ke",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct EtimsConfigPatch {
    pub enabled: Option<bool>,
    pub environment: Option<String>,
    pub pin: Option<String>,
    pub bhf_id: Option<String>,
    pub dvc_srl_no: Option<String>,
}

/// Load the entity's device row, creating a disabled default if none exists.
pub async fn get_device(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<EtimsDeviceRow> {
    if let Some(row) = sqlx::query_as::<_, EtimsDeviceRow>("SELECT * FROM etims_devices WHERE entity_id=$1")
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
    {
        return Ok(row);
    }
    // Seed the KRA PIN from the entity's tax settings when we have it.
    let pin: Option<String> = sqlx::query_scalar("SELECT kra_pin FROM entity_settings WHERE entity_id=$1")
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await
        .ok()
        .flatten();
    let row = sqlx::query_as::<_, EtimsDeviceRow>(
        "INSERT INTO etims_devices (entity_id, pin) VALUES ($1,$2) RETURNING *",
    )
    .bind(entity_id)
    .bind(pin)
    .fetch_one(engine.pool())
    .await?;
    Ok(row)
}

pub async fn save_config(engine: &ErpEngine, entity_id: Uuid, patch: EtimsConfigPatch) -> ErpResult<EtimsDeviceRow> {
    get_device(engine, entity_id).await?; // ensure the row exists
    let row = sqlx::query_as::<_, EtimsDeviceRow>(
        "UPDATE etims_devices SET
           enabled     = COALESCE($2, enabled),
           environment = COALESCE($3, environment),
           pin         = COALESCE($4, pin),
           bhf_id      = COALESCE($5, bhf_id),
           dvc_srl_no  = COALESCE($6, dvc_srl_no),
           updated_at  = now()
         WHERE entity_id=$1 RETURNING *",
    )
    .bind(entity_id)
    .bind(patch.enabled)
    .bind(patch.environment)
    .bind(patch.pin.map(|p| p.trim().to_uppercase()))
    .bind(patch.bhf_id)
    .bind(patch.dvc_srl_no)
    .fetch_one(engine.pool())
    .await?;
    Ok(row)
}

// ─── HTTP client ─────────────────────────────────────────────────────────────

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

/// KRA response envelope. `resultCd == "000"` means success.
#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "resultCd")]
    result_cd: String,
    #[serde(rename = "resultMsg")]
    result_msg: Option<String>,
    data: Option<serde_json::Value>,
}

async fn post(dev: &EtimsDeviceRow, path: &str, body: serde_json::Value, authed: bool) -> ErpResult<serde_json::Value> {
    let url = format!("{}/{}", dev.base_url(), path.trim_start_matches('/'));
    let mut req = http().post(&url).json(&body);
    if authed {
        req = req
            .header("tin", dev.pin.clone().unwrap_or_default())
            .header("bhfId", dev.bhf_id.clone())
            .header("cmcKey", dev.cmc_key.clone().unwrap_or_default());
    }
    let resp = req.send().await.map_err(|e| ErpError::ValidationFailed {
        message: format!("eTIMS network error: {e}"),
    })?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let env: Envelope = serde_json::from_str(&text).map_err(|_| ErpError::ValidationFailed {
        message: format!("eTIMS returned HTTP {status}: {}", text.chars().take(300).collect::<String>()),
    })?;
    if env.result_cd != "000" {
        return Err(ErpError::ValidationFailed {
            message: format!("eTIMS rejected the request [{}]: {}", env.result_cd, env.result_msg.unwrap_or_default()),
        });
    }
    Ok(env.data.unwrap_or(serde_json::Value::Null))
}

// ─── Device initialisation ───────────────────────────────────────────────────

pub async fn initialize_device(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<EtimsDeviceRow> {
    let dev = get_device(engine, entity_id).await?;
    let pin = dev.pin.clone().filter(|p| !p.is_empty()).ok_or_else(|| ErpError::ValidationFailed {
        message: "Set the taxpayer PIN before initialising eTIMS.".into(),
    })?;
    let serial = dev.dvc_srl_no.clone().filter(|s| !s.is_empty()).ok_or_else(|| ErpError::ValidationFailed {
        message: "Set the device serial number before initialising eTIMS.".into(),
    })?;

    let data = post(&dev, "selectInitOsdcInfo", json!({
        "tin": pin, "bhfId": dev.bhf_id, "dvcSrlNo": serial,
    }), false).await?;

    // KRA nests the device details under `info`.
    let info = data.get("info").cloned().unwrap_or(data);
    let sdc_id = info.get("sdcId").and_then(|v| v.as_str()).map(String::from);
    let mrc_no = info.get("mrcNo").and_then(|v| v.as_str()).map(String::from);
    let cmc_key = info.get("cmcKey").and_then(|v| v.as_str()).map(String::from);

    let row = sqlx::query_as::<_, EtimsDeviceRow>(
        "UPDATE etims_devices SET sdc_id=$2, mrc_no=$3, cmc_key=COALESCE($4, cmc_key),
           initialized=TRUE, initialized_at=now(), last_error=NULL, updated_at=now()
         WHERE entity_id=$1 RETURNING *",
    )
    .bind(entity_id)
    .bind(sdc_id)
    .bind(mrc_no)
    .bind(cmc_key)
    .fetch_one(engine.pool())
    .await?;
    Ok(row)
}

// ─── Tax-code mapping (KRA A/B/C/D/E) ────────────────────────────────────────

fn tax_code(t: VatTreatment) -> &'static str {
    match t {
        VatTreatment::Exempt => "A",
        VatTreatment::Standard16 => "B",
        VatTreatment::ZeroRated => "C",
        VatTreatment::OutOfScope => "D",
        VatTreatment::Petroleum8 => "E",
    }
}

fn f2(d: Decimal) -> f64 {
    d.round_dp(2).to_f64().unwrap_or(0.0)
}

/// A stable ≤20-char KRA item code derived from the product id, used
/// identically at registration and at sale so KRA can match them.
fn item_code(pid: Uuid) -> String {
    format!("ZV{}", &pid.simple().to_string()[..18])
}

// ─── Item registration (saveItem) ────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ProductRow {
    name: String,
    product_type: String,
    unit_price: Option<Decimal>,
    vat_treatment: String,
}

/// Register (or update) a product with KRA so it can appear on transmitted
/// sales. Idempotent — records the outcome in `etims_item_registry`.
pub async fn register_item(engine: &ErpEngine, entity_id: Uuid, product_id: Uuid) -> ErpResult<()> {
    let dev = get_device(engine, entity_id).await?;
    if !dev.enabled || !dev.initialized {
        return Err(ErpError::ValidationFailed { message: "eTIMS is not enabled/initialised.".into() });
    }
    let p = sqlx::query_as::<_, ProductRow>(
        "SELECT name, product_type, unit_price, vat_treatment FROM products WHERE id=$1 AND entity_id=$2",
    )
    .bind(product_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Product".into(), id: product_id })?;

    let vt: VatTreatment = serde_json::from_str(&format!("\"{}\"", p.vat_treatment)).unwrap_or(VatTreatment::Standard16);
    let item_cd = item_code(product_id);
    // Existing classification, if the product was classified before.
    let item_cls: Option<String> = sqlx::query_scalar("SELECT item_cls_cd FROM etims_item_registry WHERE entity_id=$1 AND product_id=$2")
        .bind(entity_id).bind(product_id).fetch_optional(engine.pool()).await.ok().flatten();
    let item_cls = item_cls.unwrap_or_else(|| DEFAULT_ITEM_CLS.to_string());
    let item_ty = if p.product_type.eq_ignore_ascii_case("service") { "3" } else { "2" };

    let body = json!({
        "tin": dev.pin, "bhfId": dev.bhf_id,
        "itemCd": item_cd, "itemClsCd": item_cls, "itemTyCd": item_ty,
        "itemNm": p.name, "itemStdNm": p.name,
        "orgnNatCd": "KE", "pkgUnitCd": "NT", "qtyUnitCd": "U",
        "taxTyCd": tax_code(vt),
        "dftPrc": f2(p.unit_price.unwrap_or(Decimal::ZERO)),
        "isrcAplcbYn": "N", "useYn": "Y",
        "regrId": "Zavora", "regrNm": "Zavora ERP", "modrId": "Zavora", "modrNm": "Zavora ERP",
    });

    let result = post(&dev, "saveItem", body, true).await;
    let (ok, err) = match &result { Ok(_) => (true, None), Err(e) => (false, Some(e.to_string())) };
    sqlx::query(
        "INSERT INTO etims_item_registry (entity_id, product_id, item_cd, item_cls_cd, registered, registered_at, last_error)
         VALUES ($1,$2,$3,$4,$5, CASE WHEN $5 THEN now() END, $6)
         ON CONFLICT (entity_id, product_id) DO UPDATE SET
           item_cd=$3, item_cls_cd=$4, registered=$5,
           registered_at=CASE WHEN $5 THEN now() ELSE etims_item_registry.registered_at END,
           last_error=$6",
    )
    .bind(entity_id).bind(product_id).bind(&item_cd).bind(&item_cls).bind(ok).bind(&err)
    .execute(engine.pool()).await?;
    result.map(|_| ())
}

/// Best-effort: register any of these products not yet on record with KRA.
/// Never fails — a registration hiccup shouldn't block a sale from transmitting.
async fn ensure_items_registered(engine: &ErpEngine, entity_id: Uuid, product_ids: &[Uuid]) {
    for pid in product_ids {
        let done: Option<bool> = sqlx::query_scalar("SELECT registered FROM etims_item_registry WHERE entity_id=$1 AND product_id=$2")
            .bind(entity_id).bind(pid).fetch_optional(engine.pool()).await.ok().flatten();
        if done != Some(true) {
            if let Err(e) = register_item(engine, entity_id, *pid).await {
                tracing::warn!("eTIMS item registration failed for {pid}: {e}");
            }
        }
    }
}

// ─── Sales transmission ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct EtimsReceipt {
    pub invc_no: i64,
    pub rcpt_no: Option<i64>,
    pub tot_rcpt_no: Option<i64>,
    pub sdc_id: Option<String>,
    pub mrc_no: Option<String>,
    pub rcpt_sign: Option<String>,
    pub intrl_data: Option<String>,
    pub vsdc_date: Option<String>,
    pub qr_url: Option<String>,
}

#[derive(sqlx::FromRow)]
struct InvHdr {
    number: String,
    issue_date: chrono::NaiveDate,
    subtotal: Decimal,
    tax_total: Decimal,
    gross_total: Decimal,
    status: String,
    etims_status: String,
    customer_id: Option<Uuid>,
    invoice_type: String,
    credit_note_for: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct InvLine {
    product_id: Option<Uuid>,
    description: String,
    quantity: Decimal,
    unit_price: Decimal,
    discount_percent: Decimal,
    vat_treatment: String,
    line_total: Decimal,
    vat_amount: Decimal,
}

/// Transmit a posted invoice to KRA eTIMS. Idempotent-ish: an already-transmitted
/// invoice is returned as-is. On any failure the invoice is marked
/// `transmission_failed` with the error, and the error is returned to the caller.
pub async fn transmit_invoice(engine: &ErpEngine, entity_id: Uuid, invoice_id: Uuid) -> ErpResult<EtimsReceipt> {
    let dev = get_device(engine, entity_id).await?;
    if !dev.enabled || !dev.initialized {
        return Err(ErpError::ValidationFailed {
            message: "eTIMS is not enabled/initialised for this business.".into(),
        });
    }

    let inv = sqlx::query_as::<_, InvHdr>(
        "SELECT number, issue_date, subtotal, tax_total, gross_total, status, etims_status, customer_id,
                invoice_type, credit_note_for
         FROM invoices WHERE id=$1 AND entity_id=$2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".into(), id: invoice_id })?;

    // A credit note is a KRA *credit/refund* receipt (rcptTyCd "R") that must
    // reference the eTIMS invoice number of the original sale it corrects.
    let is_credit = inv.invoice_type == "credit_note";
    let (rcpt_ty_cd, org_invc_no): (&str, i64) = if is_credit {
        let orig = inv.credit_note_for.ok_or_else(|| ErpError::ValidationFailed {
            message: "Credit note has no original invoice to reference for eTIMS.".into(),
        })?;
        let org_no: Option<i64> = sqlx::query_scalar("SELECT etims_invc_no FROM invoices WHERE id=$1 AND entity_id=$2")
            .bind(orig).bind(entity_id).fetch_optional(engine.pool()).await?.flatten();
        match org_no {
            Some(n) => ("R", n),
            None => return Err(ErpError::ValidationFailed {
                message: "Transmit the original invoice to KRA before its credit note.".into(),
            }),
        }
    } else {
        ("S", 0)
    };

    if inv.etims_status == "transmitted" {
        return Err(ErpError::ValidationFailed { message: "Invoice already transmitted to KRA.".into() });
    }
    if inv.status == "draft" {
        return Err(ErpError::ValidationFailed { message: "Post the invoice before transmitting to KRA.".into() });
    }

    let lines = sqlx::query_as::<_, InvLine>(
        "SELECT product_id, description, quantity, unit_price, discount_percent, vat_treatment, line_total, vat_amount
         FROM invoice_lines WHERE invoice_id=$1 ORDER BY id",
    )
    .bind(invoice_id)
    .fetch_all(engine.pool())
    .await?;

    let (cust_nm, cust_tin) = if let Some(cid) = inv.customer_id {
        sqlx::query_as::<_, (String, Option<String>)>("SELECT name, kra_pin FROM customers WHERE id=$1")
            .bind(cid)
            .fetch_optional(engine.pool())
            .await?
            .map(|(n, p)| (Some(n), p))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    // Make sure every product on this invoice is registered with KRA first.
    let product_ids: Vec<Uuid> = lines.iter().filter_map(|l| l.product_id).collect();
    if !product_ids.is_empty() {
        ensure_items_registered(engine, entity_id, &product_ids).await;
    }

    // Next monotonic invoice number for this branch.
    let invc_no = dev.last_invc_no + 1;

    // Per-code aggregates + item list.
    let now = Utc::now();
    let mut taxbl = [0f64; 5]; // A,B,C,D,E
    let mut taxamt = [0f64; 5];
    let idx = |c: &str| match c { "A" => 0, "B" => 1, "C" => 2, "D" => 3, _ => 4 };
    let mut item_list = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let vt: VatTreatment = serde_json::from_str(&format!("\"{}\"", l.vat_treatment)).unwrap_or(VatTreatment::Standard16);
        let code = tax_code(vt);
        let gross_line = l.line_total + l.vat_amount; // tax-inclusive total
        taxbl[idx(code)] += f2(l.line_total);
        taxamt[idx(code)] += f2(l.vat_amount);
        let dc_amt = (l.unit_price * l.quantity) * (l.discount_percent / Decimal::ONE_HUNDRED);
        item_list.push(json!({
            "itemSeq": i as i64 + 1,
            "itemCd": l.product_id.map(item_code).unwrap_or_else(|| format!("ITEM{:04}", i + 1)),
            "itemClsCd": DEFAULT_ITEM_CLS,
            "itemNm": l.description,
            "pkgUnitCd": "NT",
            "pkg": 1,
            "qtyUnitCd": "U",
            "qty": f2(l.quantity),
            "prc": f2(l.unit_price),
            "splyAmt": f2(l.line_total),
            "dcRt": f2(l.discount_percent),
            "dcAmt": f2(dc_amt),
            "taxTyCd": code,
            "taxblAmt": f2(l.line_total),
            "taxAmt": f2(l.vat_amount),
            "totAmt": f2(gross_line),
        }));
    }

    let body = json!({
        "tin": dev.pin, "bhfId": dev.bhf_id,
        "invcNo": invc_no, "orgInvcNo": org_invc_no,
        "custTin": cust_tin, "custNm": cust_nm,
        "salesTyCd": "N", "rcptTyCd": rcpt_ty_cd, "pmtTyCd": "01", "salesSttsCd": "02",
        "cfmDt": now.format("%Y%m%d%H%M%S").to_string(),
        "salesDt": inv.issue_date.format("%Y%m%d").to_string(),
        "stockRlsDt": now.format("%Y%m%d%H%M%S").to_string(),
        "totItemCnt": lines.len() as i64,
        "taxblAmtA": taxbl[0], "taxblAmtB": taxbl[1], "taxblAmtC": taxbl[2], "taxblAmtD": taxbl[3], "taxblAmtE": taxbl[4],
        "taxRtA": 0, "taxRtB": 16, "taxRtC": 0, "taxRtD": 0, "taxRtE": 8,
        "taxAmtA": taxamt[0], "taxAmtB": taxamt[1], "taxAmtC": taxamt[2], "taxAmtD": taxamt[3], "taxAmtE": taxamt[4],
        "totTaxblAmt": f2(inv.subtotal),
        "totTaxAmt": f2(inv.tax_total),
        "totAmt": f2(inv.gross_total),
        "prchrAcptcYn": "N",
        "remark": inv.number,
        "regrId": "Zavora", "regrNm": "Zavora ERP", "modrId": "Zavora", "modrNm": "Zavora ERP",
        "receipt": {
            "custTin": cust_tin, "custMblNo": serde_json::Value::Null,
            "rptNo": invc_no, "trdeNm": serde_json::Value::Null,
            "topMsg": "Welcome", "btmMsg": "Thank you", "prchrAcptcYn": "N",
        },
        "itemList": item_list,
    });

    match post(&dev, "saveTrnsSalesOsdc", body, true).await {
        Ok(data) => {
            let rcpt = EtimsReceipt {
                invc_no,
                rcpt_no: data.get("curRcptNo").and_then(|v| v.as_i64()),
                tot_rcpt_no: data.get("totRcptNo").and_then(|v| v.as_i64()),
                sdc_id: data.get("sdcId").and_then(|v| v.as_str()).map(String::from).or(dev.sdc_id.clone()),
                mrc_no: data.get("mrcNo").and_then(|v| v.as_str()).map(String::from).or(dev.mrc_no.clone()),
                rcpt_sign: data.get("rcptSign").and_then(|v| v.as_str()).map(String::from),
                intrl_data: data.get("intrlData").and_then(|v| v.as_str()).map(String::from),
                vsdc_date: data.get("vsdcRcptPbctDate").and_then(|v| v.as_str()).map(String::from),
                qr_url: None,
            };
            let qr = rcpt.rcpt_sign.as_ref().map(|sign| {
                format!("{}/common/link/etims/receipt/indexEtimsReceiptData?Data={}{}{}",
                    dev.verify_host(), dev.pin.clone().unwrap_or_default(), dev.bhf_id, sign)
            });
            sqlx::query(
                "UPDATE invoices SET etims_status='transmitted', etims_transmitted_at=now(),
                   etims_invc_no=$2, etims_rcpt_no=$3, etims_tot_rcpt_no=$4, etims_sdc_id=$5, etims_mrc_no=$6,
                   etims_rcpt_sign=$7, etims_intrl_data=$8, etims_vsdc_date=$9, etims_qr_url=$10, etims_error=NULL
                 WHERE id=$1 AND entity_id=$11",
            )
            .bind(invoice_id).bind(invc_no).bind(rcpt.rcpt_no).bind(rcpt.tot_rcpt_no)
            .bind(&rcpt.sdc_id).bind(&rcpt.mrc_no).bind(&rcpt.rcpt_sign).bind(&rcpt.intrl_data)
            .bind(&rcpt.vsdc_date).bind(&qr).bind(entity_id)
            .execute(engine.pool()).await?;
            // Advance the branch invoice counter only on success.
            sqlx::query("UPDATE etims_devices SET last_invc_no=$2, last_error=NULL, updated_at=now() WHERE entity_id=$1")
                .bind(entity_id).bind(invc_no).execute(engine.pool()).await?;
            Ok(EtimsReceipt { qr_url: qr, ..rcpt })
        }
        Err(e) => {
            let msg = e.to_string();
            sqlx::query("UPDATE invoices SET etims_status='transmission_failed', etims_error=$2 WHERE id=$1 AND entity_id=$3")
                .bind(invoice_id).bind(&msg).bind(entity_id)
                .execute(engine.pool()).await.ok();
            sqlx::query("UPDATE etims_devices SET last_error=$2, updated_at=now() WHERE entity_id=$1")
                .bind(entity_id).bind(&msg).execute(engine.pool()).await.ok();
            Err(e)
        }
    }
}

/// Best-effort auto-transmit used by the posting spine: silently no-ops when
/// eTIMS isn't configured, and never propagates a transmission error (the
/// invoice is still posted locally and can be retried).
pub async fn try_auto_transmit(engine: &ErpEngine, entity_id: Uuid, invoice_id: Uuid) {
    match get_device(engine, entity_id).await {
        Ok(dev) if dev.enabled && dev.initialized => {
            if let Err(e) = transmit_invoice(engine, entity_id, invoice_id).await {
                tracing::warn!("eTIMS auto-transmit failed for invoice {invoice_id}: {e}");
                // Reactive trigger: tell Amos what just failed so its eTIMS
                // sweep can retry and report — the owner hears about a
                // compliance gap minutes after it happens, not at 18:00.
                notify_amos_webhook(
                    "etims-sweep",
                    format!("Event: eTIMS auto-transmit failed for invoice {invoice_id}: {e}. Check the device status, retry this invoice, and report the outcome."),
                );
            }
        }
        _ => {}
    }
}

/// Best-effort fire-and-forget notification to the Amos ambient-ops trigger
/// endpoint. No-op unless AMOS_WEBHOOK_URL (+ AMOS_WEBHOOK_SECRET) are set;
/// never blocks or fails the calling posting path.
fn notify_amos_webhook(routine: &str, context: String) {
    let (Ok(base), Ok(secret)) = (std::env::var("AMOS_WEBHOOK_URL"), std::env::var("AMOS_WEBHOOK_SECRET")) else {
        return;
    };
    let url = format!("{}/api/ops/run/{routine}", base.trim_end_matches('/'));
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        match client
            .post(&url)
            .header("X-Amos-Webhook-Secret", secret)
            .json(&serde_json::json!({ "context": context }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("amos webhook: triggered {url}");
            }
            // 409 = the routine is already running (Skip policy) — fine.
            Ok(resp) if resp.status() == reqwest::StatusCode::CONFLICT => {}
            Ok(resp) => tracing::warn!("amos webhook: {url} returned {}", resp.status()),
            Err(e) => tracing::warn!("amos webhook: {url} unreachable: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_vat_treatments_to_kra_tax_codes() {
        assert_eq!(tax_code(VatTreatment::Standard16), "B");
        assert_eq!(tax_code(VatTreatment::Petroleum8), "E");
        assert_eq!(tax_code(VatTreatment::ZeroRated), "C");
        assert_eq!(tax_code(VatTreatment::Exempt), "A");
        assert_eq!(tax_code(VatTreatment::OutOfScope), "D");
    }

    fn test_device(base: &str) -> EtimsDeviceRow {
        unsafe { std::env::set_var("ETIMS_BASE_URL", base) };
        EtimsDeviceRow {
            entity_id: Uuid::nil(), enabled: true, environment: "sandbox".into(),
            pin: Some("P051234567X".into()), bhf_id: "00".into(), dvc_srl_no: Some("ZAVORA001".into()),
            sdc_id: Some("SDC001".into()), mrc_no: Some("MRC001".into()), cmc_key: Some("CMCKEY".into()),
            initialized: true, initialized_at: None, last_invc_no: 0, last_error: None,
            created_at: Utc::now(), updated_at: Utc::now(),
        }
    }

    /// End-to-end proof of the HTTP + envelope layer: post a sales request to a
    /// local mock standing in for KRA and confirm we send it and parse the SCU
    /// receipt data (result code, receipt number, signature) back.
    #[tokio::test]
    async fn transmits_and_parses_scu_receipt_from_mock_kra() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await; // drain the request
                let body = r#"{"resultCd":"000","resultMsg":"It is succeeded","data":{"curRcptNo":42,"totRcptNo":42,"rcptSign":"ABCD1234EFGH5678","intrlData":"WXYZ9876","sdcId":"SDC0010000001","mrcNo":"MRC001","vsdcRcptPbctDate":"20260707120000"}}"#;
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });

        let dev = test_device(&format!("http://127.0.0.1:{port}"));
        let data = post(&dev, "saveTrnsSalesOsdc", json!({"invcNo": 1, "totAmt": 100.0}), true).await.unwrap();
        assert_eq!(data.get("curRcptNo").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(data.get("rcptSign").and_then(|v| v.as_str()), Some("ABCD1234EFGH5678"));
        assert_eq!(data.get("sdcId").and_then(|v| v.as_str()), Some("SDC0010000001"));
        unsafe { std::env::remove_var("ETIMS_BASE_URL") };
    }

    #[test]
    fn base_url_follows_environment() {
        let mut d = EtimsDeviceRow {
            entity_id: Uuid::nil(), enabled: true, environment: "sandbox".into(), pin: None,
            bhf_id: "00".into(), dvc_srl_no: None, sdc_id: None, mrc_no: None, cmc_key: None,
            initialized: false, initialized_at: None, last_invc_no: 0, last_error: None,
            created_at: Utc::now(), updated_at: Utc::now(),
        };
        unsafe { std::env::remove_var("ETIMS_BASE_URL") };
        assert!(d.base_url().contains("sbx"));
        d.environment = "production".into();
        assert!(!d.base_url().contains("sbx"));
    }
}
