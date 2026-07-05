//! The **purchase-order (LPO) document** — the single source of truth for the
//! on-screen preview, the downloaded PDF, and (future) the emailed copy.
//!
//! A purchase order is a legal instrument: the buyer's binding offer to a
//! supplier, routinely presented to banks for LPO/invoice-discounting finance.
//! So the layout is deliberately formal — clear Buyer and Supplier blocks, the
//! LPO number and dates, priced line items, an order total, and an authorised
//! signature line. The CSS is the same self-contained A4 sheet used by the
//! invoice document (`invoicing::document`) so both read as one house style.

use rust_decimal::Decimal;

/// Everything the PO document needs. Free of DB types for easy testing/reuse.
pub struct PurchaseOrderDocument {
    // Buyer / issuer branding
    pub org_name: String,
    pub org_kra_pin: Option<String>,
    pub org_vat_number: Option<String>,
    pub org_address: Option<String>,
    pub org_email: Option<String>,
    pub org_phone: Option<String>,
    pub logo_url: Option<String>,
    pub primary_color: String, // hex like #1a56db
    pub footer_text: Option<String>,

    // Document
    pub number: String,
    pub issue_date: String,
    pub delivery_date: String, // pre-formatted or "—"
    pub currency: String,

    // Supplier (the vendor the LPO is issued to)
    pub supplier_name: String,
    pub supplier_address: Option<String>,
    pub supplier_kra_pin: Option<String>,

    // Where goods/services are delivered (defaults to the buyer's address)
    pub deliver_to: Option<String>,

    pub lines: Vec<PurchaseOrderDocLine>,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub status: String, // issued|invoiced|…, shown as a badge
    pub notes: Option<String>,
}

pub struct PurchaseOrderDocLine {
    pub description: String,
    pub quantity: Decimal,
    pub uom: String,
    pub unit_price: Decimal,
    pub line_total: Decimal,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn money(cur: &str, v: Decimal) -> String {
    // Group thousands with commas, two decimals — matches the invoice document.
    let neg = v.is_sign_negative();
    let abs = if neg { -v } else { v };
    let rounded = abs.round_dp(2);
    let s = format!("{rounded:.2}");
    let (int_part, dec_part) = s.split_once('.').unwrap_or((s.as_str(), "00"));
    let mut grouped = String::new();
    let digits: Vec<char> = int_part.chars().collect();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*c);
    }
    format!("{}{} {}.{}", if neg { "-" } else { "" }, esc(cur), grouped, dec_part)
}

/// Render the LPO as a fully self-contained HTML document (A4, inline CSS).
pub fn render_po_html(doc: &PurchaseOrderDocument) -> String {
    let accent = esc(&doc.primary_color);
    let cur = &doc.currency;

    let logo = doc
        .logo_url
        .as_deref()
        .filter(|u| !u.is_empty())
        .map(|u| format!("<img class=\"logo\" src=\"{}\" alt=\"\"/>", esc(u)))
        .unwrap_or_default();

    let org_meta = [
        doc.org_kra_pin.as_ref().map(|v| format!("KRA PIN: {}", esc(v))),
        doc.org_vat_number.as_ref().map(|v| format!("VAT: {}", esc(v))),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("  ·  ");

    let org_contact = [
        doc.org_address.as_ref().map(|v| esc(v)),
        doc.org_email.as_ref().map(|v| esc(v)),
        doc.org_phone.as_ref().map(|v| esc(v)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("  ·  ");

    let sup_addr = doc
        .supplier_address
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<div class=\"muted\">{}</div>", esc(s)))
        .unwrap_or_default();
    let sup_pin = doc
        .supplier_kra_pin
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<div class=\"muted\">PIN: {}</div>", esc(s)))
        .unwrap_or_default();

    let deliver_to = doc
        .deliver_to
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<div class=\"muted\">{}</div>", esc(s)))
        .unwrap_or_else(|| "<div class=\"muted\">As per buyer's instruction</div>".to_string());

    let mut rows = String::new();
    for l in &doc.lines {
        rows.push_str(&format!(
            "<tr><td class=\"desc\">{}</td><td class=\"num\">{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            esc(&l.description),
            esc(&l.quantity.normalize().to_string()),
            esc(&l.uom),
            money(cur, l.unit_price),
            money(cur, l.line_total),
        ));
    }

    let total_rows = format!(
        "<tr><td>Subtotal</td><td class=\"num\">{}</td></tr>\
         <tr><td>VAT</td><td class=\"num\">{}</td></tr>\
         <tr class=\"balance\"><td>Order Total</td><td class=\"num\">{}</td></tr>",
        money(cur, doc.subtotal),
        money(cur, doc.tax_total),
        money(cur, doc.gross_total),
    );

    let notes = doc
        .notes
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("<div class=\"notes\"><h4>Notes / Terms</h4><p>{}</p></div>", esc(s)))
        .unwrap_or_default();

    let footer = doc
        .footer_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(esc)
        .unwrap_or_else(|| "This is a computer-generated Local Purchase Order and is valid without a wet-ink signature. Deliver strictly against the LPO number quoted above.".to_string());

    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"/>
<style>
  @page {{ size: A4; margin: 0; }}
  * {{ box-sizing: border-box; }}
  html, body {{ margin: 0; padding: 0; }}
  body {{ font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif; color: #1f2937; background: #fff; font-size: 12px; line-height: 1.5; -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
  .page {{ width: 210mm; min-height: 297mm; margin: 0 auto; padding: 24mm 18mm; background: #fff; }}
  .muted {{ color: #6b7280; font-size: 11px; }}
  .label {{ font-size: 9px; font-weight: 700; text-transform: uppercase; letter-spacing: .8px; color: #9ca3af; margin-bottom: 6px; }}
  .num {{ text-align: right; white-space: nowrap; }}

  /* Header */
  .head {{ display: flex; justify-content: space-between; align-items: flex-start; gap: 24px; padding-bottom: 22px; border-bottom: 2px solid {accent}; }}
  .brand {{ display: flex; gap: 14px; align-items: flex-start; }}
  .logo {{ height: 52px; width: auto; object-fit: contain; }}
  .org-name {{ font-size: 18px; font-weight: 700; color: {accent}; letter-spacing: .2px; }}
  .head .doc {{ text-align: right; min-width: 230px; }}
  .doc-title {{ font-size: 24px; font-weight: 800; letter-spacing: 2px; color: #111827; text-transform: uppercase; }}
  .doc-sub {{ font-size: 10px; color: #9ca3af; letter-spacing: 1px; text-transform: uppercase; margin-top: 2px; }}
  table.meta {{ margin-left: auto; margin-top: 10px; border-collapse: collapse; }}
  table.meta td {{ padding: 2px 0; font-size: 11px; }}
  table.meta td.k {{ color: #9ca3af; text-align: right; padding-right: 12px; text-transform: uppercase; letter-spacing: .4px; font-size: 9.5px; }}
  table.meta td.v {{ font-weight: 600; text-align: right; color: #111827; }}
  .badge {{ display: inline-block; margin-top: 8px; padding: 3px 10px; border-radius: 999px; font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: .5px; background: #eef2ff; color: {accent}; }}

  /* Parties */
  .info {{ display: flex; justify-content: space-between; align-items: stretch; gap: 24px; margin-top: 26px; }}
  .party {{ flex: 1; }}
  .party .name {{ font-size: 14px; font-weight: 700; color: #111827; }}

  /* Items */
  table.items {{ width: 100%; border-collapse: collapse; margin-top: 22px; }}
  table.items th {{ text-align: left; font-size: 9.5px; font-weight: 700; text-transform: uppercase; letter-spacing: .6px; color: #6b7280; background: #f9fafb; padding: 10px 10px; border-bottom: 1.5px solid {accent}; }}
  table.items th.num {{ text-align: right; }}
  table.items td {{ font-size: 12px; padding: 11px 10px; border-bottom: 1px solid #eef0f2; vertical-align: top; }}
  table.items td.desc {{ color: #111827; }}
  table.items tbody tr:nth-child(even) td {{ background: #fbfcfd; }}

  /* Totals */
  .totals {{ display: flex; justify-content: flex-end; margin-top: 18px; }}
  table.tot {{ min-width: 290px; border-collapse: collapse; }}
  table.tot td {{ padding: 6px 10px; font-size: 12px; color: #374151; }}
  table.tot td.num {{ text-align: right; font-weight: 600; color: #111827; }}
  table.tot tr.balance td {{ background: #f9fafb; border-top: 2px solid {accent}; font-weight: 800; font-size: 14px; color: {accent}; padding: 10px; }}

  /* Notes */
  .notes {{ margin-top: 26px; padding: 14px 16px; background: #f9fafb; border-left: 3px solid {accent}; border-radius: 0 6px 6px 0; }}
  .notes h4 {{ margin: 0 0 4px; font-size: 9.5px; font-weight: 700; text-transform: uppercase; letter-spacing: .6px; color: #9ca3af; }}
  .notes p {{ margin: 0; font-size: 12px; color: #374151; white-space: pre-line; }}

  /* Authorisation */
  .auth {{ display: flex; justify-content: space-between; gap: 40px; margin-top: 46px; }}
  .sig {{ flex: 1; }}
  .sig .line {{ border-top: 1px solid #9ca3af; margin-top: 40px; padding-top: 6px; font-size: 10px; color: #6b7280; text-transform: uppercase; letter-spacing: .5px; }}

  .foot {{ margin-top: 30px; padding-top: 14px; border-top: 1px solid #e5e7eb; text-align: center; color: #9ca3af; font-size: 10.5px; }}
</style></head>
<body><div class="page">
  <div class="head">
    <div class="brand">
      {logo}
      <div>
        <div class="org-name">{org}</div>
        <div class="muted">{org_contact}</div>
        <div class="muted">{org_meta}</div>
      </div>
    </div>
    <div class="doc">
      <div class="doc-title">Purchase Order</div>
      <div class="doc-sub">Local Purchase Order (LPO)</div>
      <table class="meta">
        <tr><td class="k">LPO No</td><td class="v">{number}</td></tr>
        <tr><td class="k">Order Date</td><td class="v">{issue}</td></tr>
        <tr><td class="k">Delivery Date</td><td class="v">{delivery}</td></tr>
        <tr><td class="k">Currency</td><td class="v">{currency}</td></tr>
      </table>
      <div class="badge">{status}</div>
    </div>
  </div>

  <div class="info">
    <div class="party">
      <div class="label">Supplier</div>
      <div class="name">{supplier}</div>
      {sup_addr}
      {sup_pin}
    </div>
    <div class="party">
      <div class="label">Deliver To</div>
      <div class="name">{org}</div>
      {deliver_to}
    </div>
  </div>

  <table class="items">
    <thead><tr><th>Description</th><th class="num">Qty</th><th>UoM</th><th class="num">Unit Price</th><th class="num">Amount</th></tr></thead>
    <tbody>{rows}</tbody>
  </table>

  <div class="totals">
    <table class="tot">
      {total_rows}
    </table>
  </div>

  {notes}

  <div class="auth">
    <div class="sig"><div class="line">Authorised by (Buyer)</div></div>
    <div class="sig"><div class="line">Accepted by (Supplier) — name, sign &amp; date</div></div>
  </div>

  <div class="foot">{footer}</div>
</div></body></html>"#,
        accent = accent,
        logo = logo,
        org = esc(&doc.org_name),
        org_meta = org_meta,
        org_contact = org_contact,
        number = esc(&doc.number),
        issue = esc(&doc.issue_date),
        delivery = esc(&doc.delivery_date),
        currency = esc(&doc.currency),
        status = esc(&doc.status),
        supplier = esc(&doc.supplier_name),
        sup_addr = sup_addr,
        sup_pin = sup_pin,
        deliver_to = deliver_to,
        rows = rows,
        total_rows = total_rows,
        notes = notes,
        footer = footer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample() -> PurchaseOrderDocument {
        PurchaseOrderDocument {
            org_name: "Zavora Technologies Ltd".into(),
            org_kra_pin: Some("P051234567X".into()),
            org_vat_number: None,
            org_address: Some("Nairobi, Kenya".into()),
            org_email: Some("ap@zavora.ai".into()),
            org_phone: None,
            logo_url: None,
            primary_color: "#1a56db".into(),
            footer_text: None,
            number: "LPO-2026-0003".into(),
            issue_date: "05 Jul 2026".into(),
            delivery_date: "15 Sep 2026".into(),
            currency: "Ksh".into(),
            supplier_name: "Acme Supplies Ltd".into(),
            supplier_address: Some("Industrial Area, Nairobi".into()),
            supplier_kra_pin: Some("P012345678Q".into()),
            deliver_to: None,
            lines: vec![PurchaseOrderDocLine {
                description: "A3 network laser printer".into(),
                quantity: dec!(10),
                uom: "unit".into(),
                unit_price: dec!(45000),
                line_total: dec!(450000),
            }],
            subtotal: dec!(450000),
            tax_total: dec!(72000),
            gross_total: dec!(522000),
            status: "issued".into(),
            notes: Some("Delivery within 2 weeks.".into()),
        }
    }

    #[test]
    fn renders_self_contained_html() {
        let html = render_po_html(&sample());
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("Purchase Order"));
        assert!(html.contains("LPO-2026-0003"));
        assert!(html.contains("Acme Supplies Ltd"));
        assert!(html.contains("Authorised by (Buyer)"));
        // Self-contained: no external stylesheet/script references.
        assert!(!html.contains("<link"));
        assert!(!html.contains("http://"));
    }

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money("Ksh", dec!(522000)), "Ksh 522,000.00");
        assert_eq!(money("Ksh", dec!(45000)), "Ksh 45,000.00");
    }
}
