//! Point of Sale — register/shift sessions and fast cash/M-Pesa sales.
//!
//! A POS sale is an orchestration over the existing spine, so the ledger stays
//! correct with zero new accounting: create a draft invoice from the cart →
//! `post_invoice` (revenue + VAT + stock issue + COGS) → `record_payment`
//! (settles AR, deposits cash/M-Pesa to the till account). Each sale is tagged
//! to the open shift so the till reconciles with a Z-report at close.

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::types::AgentOrUserId;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PosSessionRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub register_name: String,
    pub opened_by: Uuid,
    pub opened_at: DateTime<Utc>,
    pub opening_float: Decimal,
    pub cash_account_id: Option<Uuid>,
    pub mpesa_account_id: Option<Uuid>,
    pub closed_by: Option<Uuid>,
    pub closed_at: Option<DateTime<Utc>>,
    pub counted_cash: Option<Decimal>,
    pub expected_cash: Option<Decimal>,
    pub cash_variance: Option<Decimal>,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenSessionRequest {
    pub register_name: Option<String>,
    #[serde(default)]
    pub opening_float: Decimal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaleLineRequest {
    pub product_id: Uuid,
    pub quantity: Decimal,
    pub unit_price: Option<Decimal>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompleteSaleRequest {
    pub customer_id: Option<Uuid>,
    pub tender: String, // cash | mpesa | card
    #[serde(default)]
    pub amount_tendered: Option<Decimal>, // for cash change calc
    pub mpesa_reference: Option<String>,
    pub mpesa_phone: Option<String>,
    pub lines: Vec<SaleLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloseSessionRequest {
    pub counted_cash: Decimal,
    pub notes: Option<String>,
}

/// Resolve a deposit bank account for a tender by a name heuristic, falling back
/// to the first KES account. cash → cash/petty/drawer/till; mpesa → mpesa/m-pesa.
async fn resolve_account(engine: &ErpEngine, entity_id: Uuid, tender: &str) -> ErpResult<Uuid> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM bank_accounts WHERE entity_id=$1")
        .bind(entity_id).fetch_all(engine.pool()).await?;
    let want: &[&str] = match tender {
        "mpesa" => &["mpesa", "m-pesa"],
        // Note: not "till" — "M-Pesa Till" would wrongly match the cash drawer.
        "cash" => &["cash", "petty", "drawer"],
        _ => &[],
    };
    if let Some((id, _)) = rows.iter().find(|(_, n)| { let l = n.to_lowercase(); want.iter().any(|w| l.contains(w)) }) {
        return Ok(*id);
    }
    rows.first().map(|(id, _)| *id)
        .ok_or_else(|| ErpError::ValidationFailed { message: "no bank/till account configured to deposit takings into".into() })
}

/// The tenant's walk-in customer (created once), used when a sale names no customer.
async fn walk_in_customer(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Uuid> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM customers WHERE entity_id=$1 AND name='Walk-in Customer' LIMIT 1")
        .bind(entity_id).fetch_optional(engine.pool()).await? {
        return Ok(id);
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO customers (id, entity_id, name, email, phone, is_active) VALUES ($1,$2,'Walk-in Customer','[]'::jsonb,'[]'::jsonb,true)")
        .bind(id).bind(entity_id).execute(engine.pool()).await?;
    Ok(id)
}

pub async fn open_session(engine: &ErpEngine, entity_id: Uuid, req: OpenSessionRequest, opened_by: Uuid) -> ErpResult<PosSessionRow> {
    if let Some(existing) = get_open_session(engine, entity_id, opened_by).await? {
        return Err(ErpError::ValidationFailed { message: format!("you already have an open till ({}). Close it first.", existing.register_name) });
    }
    let cash = resolve_account(engine, entity_id, "cash").await.ok();
    let mpesa = resolve_account(engine, entity_id, "mpesa").await.ok();
    let id = Uuid::new_v4();
    let row = sqlx::query_as::<_, PosSessionRow>(
        r#"INSERT INTO pos_sessions (id, entity_id, register_name, opened_by, opening_float, cash_account_id, mpesa_account_id, status)
           VALUES ($1,$2,$3,$4,$5,$6,$7,'open') RETURNING *"#,
    )
    .bind(id).bind(entity_id).bind(req.register_name.unwrap_or_else(|| "Main Till".into()))
    .bind(opened_by).bind(req.opening_float).bind(cash).bind(mpesa)
    .fetch_one(engine.pool()).await?;
    let _ = crate::services::audit::record_event(engine, entity_id, "Opened", "pos_session", id,
        &AgentOrUserId::User(opened_by), Some(serde_json::json!({ "opening_float": req.opening_float }))).await;
    Ok(row)
}

pub async fn get_open_session(engine: &ErpEngine, entity_id: Uuid, user: Uuid) -> ErpResult<Option<PosSessionRow>> {
    Ok(sqlx::query_as::<_, PosSessionRow>("SELECT * FROM pos_sessions WHERE entity_id=$1 AND opened_by=$2 AND status='open' ORDER BY opened_at DESC LIMIT 1")
        .bind(entity_id).bind(user).fetch_optional(engine.pool()).await?)
}

pub async fn list_sessions(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<PosSessionRow>> {
    Ok(sqlx::query_as::<_, PosSessionRow>("SELECT * FROM pos_sessions WHERE entity_id=$1 ORDER BY opened_at DESC LIMIT 100")
        .bind(entity_id).fetch_all(engine.pool()).await?)
}

/// Complete a POS sale end to end and tie it to the open shift.
pub async fn complete_sale(engine: &ErpEngine, entity_id: Uuid, session_id: Uuid, req: CompleteSaleRequest, cashier: Uuid) -> ErpResult<serde_json::Value> {
    use crate::invoicing::invoice::CreateInvoiceRequest;
    use crate::invoicing::line::CreateInvoiceLineRequest;
    use crate::payments::payment::{PaymentApplicationRequest, PaymentMethod, PaymentType, RecordPaymentRequest};

    let session = sqlx::query_as::<_, PosSessionRow>("SELECT * FROM pos_sessions WHERE id=$1 AND entity_id=$2 AND status='open'")
        .bind(session_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::ValidationFailed { message: "no open till — open a shift before selling".into() })?;
    if req.lines.is_empty() {
        return Err(ErpError::ValidationFailed { message: "the cart is empty".into() });
    }

    let customer_id = match req.customer_id { Some(c) => c, None => walk_in_customer(engine, entity_id).await? };
    let actor = AgentOrUserId::User(cashier);

    // 1) Draft invoice from the cart.
    let invoice = crate::services::invoicing::create_invoice(engine, entity_id, CreateInvoiceRequest {
        customer_id,
        issue_date: None,
        due_date: None,
        currency: None,
        fx_rate: None,
        template_id: None,
        notes: Some(format!("POS sale — {}", session.register_name)),
        send_immediately: Some(false),
        lines: req.lines.iter().map(|l| CreateInvoiceLineRequest {
            product_id: Some(l.product_id),
            description: None,
            quantity: l.quantity,
            unit_price: l.unit_price,
            discount_percent: None,
            account_code: None,
            vat_treatment: None,
            dimensions: None,
        }).collect(),
    }, &actor).await?;

    // 2) Post it — issues stock, posts revenue + VAT + COGS.
    crate::services::invoicing::post_invoice(engine, entity_id, invoice.id, &actor).await?;
    let gross: Decimal = sqlx::query_scalar("SELECT gross_total FROM invoices WHERE id=$1").bind(invoice.id).fetch_one(engine.pool()).await?;

    // 3) Tender → deposit account + payment method.
    let (bank_account_id, method) = match req.tender.as_str() {
        "cash" => (session.cash_account_id, PaymentMethod::Cash),
        "mpesa" => (session.mpesa_account_id, PaymentMethod::Mpesa {
            transaction_id: req.mpesa_reference.clone().unwrap_or_default(),
            phone: req.mpesa_phone.clone().unwrap_or_default(),
        }),
        "card" => (session.cash_account_id, PaymentMethod::Card { processor: "POS".into(), authorization: req.mpesa_reference.clone().unwrap_or_default() }),
        other => return Err(ErpError::ValidationFailed { message: format!("unknown tender '{other}'") }),
    };
    let bank_account_id = match bank_account_id { Some(b) => Some(b), None => Some(resolve_account(engine, entity_id, &req.tender).await?) };

    // 4) Record the receipt (settles AR in full).
    let payment = crate::services::payments::record_payment(engine, entity_id, RecordPaymentRequest {
        payment_type: PaymentType::CustomerPayment,
        party_id: customer_id,
        payment_date: None,
        amount: gross,
        currency: None,
        fx_rate: None,
        method,
        reference: req.mpesa_reference.clone(),
        bank_account_id,
        applications: vec![PaymentApplicationRequest { document_id: invoice.id, amount: gross }],
        wht_amount: None,
        wht_account: None,
        funding_account: None,
    }, &actor).await?;

    // 5) Tag the sale to the shift.
    sqlx::query("INSERT INTO pos_sales (entity_id, session_id, invoice_id, payment_id, tender, amount) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(entity_id).bind(session_id).bind(invoice.id).bind(payment.id).bind(&req.tender).bind(gross)
        .execute(engine.pool()).await?;

    let change = if req.tender == "cash" {
        req.amount_tendered.map(|t| (t - gross).max(Decimal::ZERO)).unwrap_or(Decimal::ZERO)
    } else { Decimal::ZERO };

    Ok(serde_json::json!({
        "invoice_id": invoice.id,
        "invoice_number": invoice.number,
        "payment_id": payment.id,
        "gross_total": gross,
        "tender": req.tender,
        "change": change,
    }))
}

/// Z-report: sales by tender within a session + expected cash in the drawer.
pub async fn z_report(engine: &ErpEngine, entity_id: Uuid, session_id: Uuid) -> ErpResult<serde_json::Value> {
    let session = sqlx::query_as::<_, PosSessionRow>("SELECT * FROM pos_sessions WHERE id=$1 AND entity_id=$2")
        .bind(session_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "pos session".into(), id: session_id })?;

    let by_tender: Vec<(String, i64, Decimal)> = sqlx::query_as(
        "SELECT tender, COUNT(*), COALESCE(SUM(amount),0) FROM pos_sales WHERE session_id=$1 GROUP BY tender",
    ).bind(session_id).fetch_all(engine.pool()).await.unwrap_or_default();

    let mut cash_sales = Decimal::ZERO;
    let mut total = Decimal::ZERO;
    let mut count = 0i64;
    let tenders: Vec<serde_json::Value> = by_tender.iter().map(|(t, c, a)| {
        total += *a; count += *c;
        if t == "cash" { cash_sales += *a; }
        serde_json::json!({ "tender": t, "count": c, "amount": a })
    }).collect();
    let expected_cash = session.opening_float + cash_sales;

    Ok(serde_json::json!({
        "session": session,
        "tenders": tenders,
        "sales_count": count,
        "gross_total": total,
        "cash_sales": cash_sales,
        "expected_cash": expected_cash,
    }))
}

pub async fn close_session(engine: &ErpEngine, entity_id: Uuid, session_id: Uuid, req: CloseSessionRequest, closed_by: Uuid) -> ErpResult<serde_json::Value> {
    let session = sqlx::query_as::<_, PosSessionRow>("SELECT * FROM pos_sessions WHERE id=$1 AND entity_id=$2")
        .bind(session_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "pos session".into(), id: session_id })?;
    let cash_sales: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(amount),0) FROM pos_sales WHERE session_id=$1 AND tender='cash'")
        .bind(session_id).fetch_one(engine.pool()).await.unwrap_or(Decimal::ZERO);
    let expected = session.opening_float + cash_sales;
    let variance = req.counted_cash - expected;
    let z = z_report(engine, entity_id, session_id).await?;

    let updated = sqlx::query("UPDATE pos_sessions SET status='closed', closed_by=$3, closed_at=now(), counted_cash=$4, expected_cash=$5, cash_variance=$6, notes=COALESCE($7, notes) WHERE id=$1 AND entity_id=$2 AND status='open'")
        .bind(session_id).bind(entity_id).bind(closed_by).bind(req.counted_cash).bind(expected).bind(variance).bind(&req.notes)
        .execute(engine.pool()).await?;
    if updated.rows_affected() == 0 {
        return Err(ErpError::ValidationFailed { message: "session not found or already closed".into() });
    }
    let _ = crate::services::audit::record_event(engine, entity_id, "Closed", "pos_session", session_id,
        &AgentOrUserId::User(closed_by), Some(serde_json::json!({ "expected": expected, "counted": req.counted_cash, "variance": variance }))).await;

    Ok(serde_json::json!({ "z_report": z, "counted_cash": req.counted_cash, "expected_cash": expected, "cash_variance": variance }))
}

// ── ETR / eTIMS tax receipt (80mm thermal) ──────────────────────────────────

fn r_esc(s: &str) -> String { s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;") }
fn r_money(v: Decimal) -> String {
    let n = v.round_dp(2); let s = format!("{n:.2}");
    let (i, d) = s.split_once('.').unwrap_or((s.as_str(), "00"));
    let neg = i.starts_with('-'); let digits: Vec<char> = i.trim_start_matches('-').chars().collect();
    let mut g = String::new();
    for (idx, c) in digits.iter().enumerate() { if idx>0 && (digits.len()-idx)%3==0 { g.push(','); } g.push(*c); }
    format!("{}{}.{}", if neg {"-"} else {""}, g, d)
}

/// Render the KRA-style ETR/eTIMS tax receipt for a POS sale as a self-contained
/// 80mm thermal HTML page (auto-prints on load). Reuses the invoice as the tax
/// document and adds the eTIMS control block + QR and the POS tender/change.
pub async fn pos_receipt_html(engine: &ErpEngine, entity_id: Uuid, invoice_id: Uuid, tendered: Option<Decimal>) -> ErpResult<String> {
    use qrcode::{QrCode, render::svg};

    #[derive(sqlx::FromRow)]
    struct RcptInv {
        number: String, issue_date: chrono::NaiveDate,
        subtotal: Decimal, tax_total: Decimal, gross_total: Decimal,
        etims_status: String,
        etims_rcpt_no: Option<i64>, etims_tot_rcpt_no: Option<i64>, etims_invc_no: Option<i64>,
        etims_sdc_id: Option<String>, etims_rcpt_sign: Option<String>,
        etims_intrl_data: Option<String>, etims_qr_url: Option<String>,
    }
    let inv = sqlx::query_as::<_, RcptInv>(
        "SELECT number, issue_date, subtotal, tax_total, gross_total, etims_status,
                etims_rcpt_no, etims_tot_rcpt_no, etims_invc_no, etims_sdc_id, etims_rcpt_sign,
                etims_intrl_data, etims_qr_url
         FROM invoices WHERE id=$1 AND entity_id=$2",
    ).bind(invoice_id).bind(entity_id).fetch_optional(engine.pool()).await?
     .ok_or_else(|| ErpError::NotFound { entity_type: "invoice".into(), id: invoice_id })?;
    let (number, issue_date, subtotal, tax_total, gross_total, etims_status) =
        (inv.number, inv.issue_date, inv.subtotal, inv.tax_total, inv.gross_total, inv.etims_status);

    let lines: Vec<(String, Decimal, Decimal, Decimal)> = sqlx::query_as(
        "SELECT description, quantity, unit_price, line_total FROM invoice_lines WHERE invoice_id=$1 ORDER BY id",
    ).bind(invoice_id).fetch_all(engine.pool()).await.unwrap_or_default();

    let (org, kra_pin, branding, tax_cfg): (Option<String>, Option<String>, Option<serde_json::Value>, Option<serde_json::Value>) =
        sqlx::query_as("SELECT organization_name, kra_pin, branding, tax_config FROM entity_settings WHERE entity_id=$1")
        .bind(entity_id).fetch_optional(engine.pool()).await?.unwrap_or((None,None,None,None));
    let b = branding.unwrap_or_default();
    let bget = |k: &str| b.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let org = bget("company_name").or(org).unwrap_or_else(|| "Your Company".into());
    let addr = bget("address"); let phone = bget("phone"); let vat_no = bget("vat_number");
    let vat_registered = tax_cfg.as_ref().and_then(|t| t.get("vat_registered")).and_then(|v| v.as_bool()).unwrap_or(false);
    let kra_pin = kra_pin.unwrap_or_default();

    let tender: Option<String> = sqlx::query_scalar("SELECT tender FROM pos_sales WHERE invoice_id=$1 LIMIT 1")
        .bind(invoice_id).fetch_optional(engine.pool()).await.ok().flatten();
    let tender = tender.unwrap_or_else(|| "cash".into());
    let tendered = tendered.unwrap_or(gross_total);
    let change = if tender == "cash" { (tendered - gross_total).max(Decimal::ZERO) } else { Decimal::ZERO };

    // eTIMS control block + QR. Once transmitted, the QR carries KRA's signed
    // verification URL and the block shows the SCU id, receipt number and the
    // internal-data / signature KRA returned. Before transmission it falls back
    // to a plain verification URL and a "pending" state.
    let transmitted = etims_status == "transmitted";
    let qr_payload = inv.etims_qr_url.clone().unwrap_or_else(||
        format!("https://etims.kra.go.ke/common/link/etims/receipt/indexEtimsReceiptData?PIN={kra_pin}&RcptNo={number}&Amt={gross_total}"));
    let qr_svg = QrCode::new(qr_payload.as_bytes())
        .map(|c| c.render::<svg::Color>().min_dimensions(150,150).quiet_zone(false).build())
        .unwrap_or_default();
    // KRA-mandated control block rows (only meaningful once transmitted).
    let etims_control = if transmitted {
        let rcpt = match (inv.etims_rcpt_no, inv.etims_tot_rcpt_no) {
            (Some(c), Some(t)) => format!("{c}/{t}"),
            (Some(c), None) => c.to_string(),
            _ => "-".into(),
        };
        format!(
            "<div>SCU ID: {sdc}</div><div>Receipt No: {rcpt}</div><div>Invoice No: {invc}</div>\
             <div style='word-break:break-all;font-size:9px'>Int. Data: {intrl}</div>\
             <div style='word-break:break-all;font-size:9px'>Signature: {sign}</div>",
            sdc = r_esc(&inv.etims_sdc_id.clone().unwrap_or_default()),
            invc = inv.etims_invc_no.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
            intrl = r_esc(&inv.etims_intrl_data.clone().unwrap_or_default()),
            sign = r_esc(&inv.etims_rcpt_sign.clone().unwrap_or_default()),
        )
    } else {
        String::new()
    };

    let mut item_rows = String::new();
    for (d, q, up, lt) in &lines {
        item_rows.push_str(&format!(
            "<div class='it'><div class='nm'>{}</div><div class='qp'><span>{} x {}</span><span>{}</span></div></div>",
            r_esc(d), q.normalize(), r_money(*up), r_money(*lt)));
    }
    let vat_line = if vat_registered {
        format!("<div class='row'><span>VAT (16%)</span><span>{}</span></div>", r_money(tax_total))
    } else { String::new() };
    let addr_l = addr.map(|a| format!("<div>{}</div>", r_esc(&a))).unwrap_or_default();
    let phone_l = phone.map(|p| format!("<div>Tel: {}</div>", r_esc(&p))).unwrap_or_default();
    let vat_l = if vat_registered { vat_no.map(|v| format!("<div>VAT No: {}</div>", r_esc(&v))).unwrap_or_default() } else { String::new() };

    Ok(format!(r#"<!DOCTYPE html><html><head><meta charset="utf-8"/>
<style>
  @page {{ size: 80mm auto; margin: 0; }}
  * {{ box-sizing: border-box; }}
  body {{ width: 80mm; margin: 0 auto; padding: 4mm; font-family: 'Courier New', monospace; font-size: 11px; color: #000; }}
  .c {{ text-align: center; }} .b {{ font-weight: 700; }}
  .hr {{ border-top: 1px dashed #000; margin: 4px 0; }}
  .row {{ display: flex; justify-content: space-between; }}
  .it {{ margin: 2px 0; }} .it .nm {{ }} .it .qp {{ display: flex; justify-content: space-between; color: #000; }}
  .tot {{ font-size: 14px; font-weight: 700; }}
  .etims {{ margin-top: 4px; padding: 4px; border: 1px solid #000; text-align: center; }}
  .etims svg {{ width: 130px; height: 130px; }}
  h1 {{ font-size: 15px; margin: 0; }}
</style></head><body onload="window.print()">
  <div class="c">
    <h1 class="b">{org}</h1>
    {addr_l}{phone_l}
    <div>PIN: {kra_pin}</div>{vat_l}
    <div class="b">{doc_title}</div>
  </div>
  <div class="hr"></div>
  <div class="row"><span>Receipt:</span><span>{number}</span></div>
  <div class="row"><span>Date:</span><span>{date}</span></div>
  <div class="hr"></div>
  {item_rows}
  <div class="hr"></div>
  <div class="row"><span>Subtotal</span><span>{subtotal}</span></div>
  {vat_line}
  <div class="row tot"><span>TOTAL</span><span>KSh {gross}</span></div>
  <div class="hr"></div>
  <div class="row"><span>{tender_label}</span><span>{tendered}</span></div>
  <div class="row"><span>Change</span><span>{change}</span></div>
  <div class="etims">
    <div class="b">eTIMS TAX RECEIPT</div>
    <div>{etims_state}</div>
    {etims_control}
    {qr_svg}
    <div style="font-size:9px">Scan to verify on KRA eTIMS</div>
  </div>
  <div class="c" style="margin-top:6px">*** Thank you — come again ***</div>
</body></html>"#,
        org = r_esc(&org),
        addr_l = addr_l, phone_l = phone_l, vat_l = vat_l,
        kra_pin = r_esc(&kra_pin),
        doc_title = if vat_registered { "TAX INVOICE / RECEIPT" } else { "SALES RECEIPT" },
        number = r_esc(&number),
        date = issue_date.format("%d/%m/%Y"),
        item_rows = item_rows,
        subtotal = r_money(subtotal),
        vat_line = vat_line,
        gross = r_money(gross_total),
        tender_label = match tender.as_str() {"mpesa"=>"M-PESA","card"=>"CARD",_=>"CASH"},
        tendered = r_money(tendered),
        change = r_money(change),
        etims_control = etims_control,
        etims_state = if transmitted { "eTIMS: Transmitted to KRA" } else { "eTIMS: Pending transmission" },
        qr_svg = qr_svg,
    ))
}
