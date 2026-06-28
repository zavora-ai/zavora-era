//! Single source of truth for the **invoice document**.
//!
//! The same HTML produced here is used in three places so they look identical:
//!   1. on-screen preview (rendered in an iframe in the UI),
//!   2. the downloaded PDF (this HTML printed to PDF), and
//!   3. the emailed PDF attachment (the same HTML printed to PDF).
//!
//! Keeping one renderer is the only way to guarantee the three match — any
//! second implementation drifts. The HTML is fully self-contained (inline CSS,
//! no external assets except an optional logo URL) and sized for A4 so the
//! print/PDF output matches the on-screen layout.

use rust_decimal::Decimal;

/// Everything the document needs. Free of DB types for easy testing/reuse.
pub struct InvoiceDocument {
    // Issuer / branding
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
    pub title: String, // "TAX INVOICE" / "CREDIT NOTE" / "PROFORMA INVOICE"
    pub number: String,
    pub issue_date: String,
    pub due_date: String,
    pub currency: String,
    pub etims_number: Option<String>,

    // Customer
    pub customer_name: String,
    pub customer_address: Option<String>,
    pub customer_kra_pin: Option<String>,

    // Lines + totals
    pub lines: Vec<InvoiceDocLine>,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub notes: Option<String>,

    // Labels + flags so one renderer serves invoices, estimates and credit notes.
    pub number_label: String,  // "Invoice No" | "Estimate No"
    pub date2_label: String,   // "Due Date"   | "Valid Until"
    pub summary_label: String, // "Balance Due"| "Total"
    pub show_payments: bool,   // show Amount Paid / Balance Due rows (false for estimates)
}

pub struct InvoiceDocLine {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub vat_amount: Decimal,
    pub line_total: Decimal,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn money(cur: &str, d: Decimal) -> String {
    let neg = d.is_sign_negative();
    let v = d.abs().round_dp(2);
    let s = format!("{v:.2}");
    let (int_part, frac) = s.split_once('.').unwrap_or((s.as_str(), "00"));
    let mut grouped = String::new();
    let bytes = int_part.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*b as char);
    }
    format!("{}{} {}{}.{}", if neg { "-" } else { "" }, esc(cur), grouped, "", frac)
}

/// Render the invoice as a complete, self-contained HTML document.
pub fn render_invoice_html(doc: &InvoiceDocument) -> String {
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

    let cust_addr = doc
        .customer_address
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<div class=\"muted\">{}</div>", esc(s)))
        .unwrap_or_default();
    let cust_pin = doc
        .customer_kra_pin
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<div class=\"muted\">PIN: {}</div>", esc(s)))
        .unwrap_or_default();

    let etims = doc
        .etims_number
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<div class=\"etims\">eTIMS Control No: {}</div>", esc(s)))
        .unwrap_or_default();

    let mut rows = String::new();
    for l in &doc.lines {
        rows.push_str(&format!(
            "<tr><td class=\"desc\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            esc(&l.description),
            esc(&l.quantity.normalize().to_string()),
            money(cur, l.unit_price),
            money(cur, l.vat_amount),
            money(cur, l.line_total),
        ));
    }

    let discount_row = if doc.discount_total > Decimal::ZERO {
        format!(
            "<tr><td>Discount</td><td class=\"num\">-{}</td></tr>",
            money(cur, doc.discount_total)
        )
    } else {
        String::new()
    };

    // Totals body. Invoices end on a prominent "Balance Due" (after an optional
    // Amount Paid row); estimates/quotes end on a prominent "Total".
    let mut total_rows = format!(
        "<tr><td>Subtotal</td><td class=\"num\">{}</td></tr>{}<tr><td>VAT</td><td class=\"num\">{}</td></tr>",
        money(cur, doc.subtotal),
        discount_row,
        money(cur, doc.tax_total),
    );
    if doc.show_payments {
        total_rows.push_str(&format!(
            "<tr class=\"total\"><td>Total</td><td class=\"num\">{}</td></tr>",
            money(cur, doc.gross_total)
        ));
        if doc.amount_paid > Decimal::ZERO {
            total_rows.push_str(&format!(
                "<tr><td>Amount Paid</td><td class=\"num paid\">{}</td></tr>",
                money(cur, doc.amount_paid)
            ));
        }
        total_rows.push_str(&format!(
            "<tr class=\"balance\"><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(&doc.summary_label),
            money(cur, doc.balance_due)
        ));
    } else {
        total_rows.push_str(&format!(
            "<tr class=\"balance\"><td>Total</td><td class=\"num\">{}</td></tr>",
            money(cur, doc.gross_total)
        ));
    }

    let notes = doc
        .notes
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("<div class=\"notes\"><h4>Notes</h4><p>{}</p></div>", esc(s)))
        .unwrap_or_default();

    let footer = doc
        .footer_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(esc)
        .unwrap_or_default();

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
  .doc-title {{ font-size: 26px; font-weight: 800; letter-spacing: 3px; color: #111827; text-transform: uppercase; }}
  table.meta {{ margin-left: auto; margin-top: 10px; border-collapse: collapse; }}
  table.meta td {{ padding: 2px 0; font-size: 11px; }}
  table.meta td.k {{ color: #9ca3af; text-align: right; padding-right: 12px; text-transform: uppercase; letter-spacing: .4px; font-size: 9.5px; }}
  table.meta td.v {{ font-weight: 600; text-align: right; color: #111827; }}

  /* Parties + amount due */
  .info {{ display: flex; justify-content: space-between; align-items: stretch; gap: 24px; margin-top: 26px; }}
  .billto .name {{ font-size: 14px; font-weight: 700; color: #111827; }}
  .due-box {{ min-width: 220px; background: {accent}; color: #fff; border-radius: 10px; padding: 16px 20px; text-align: right; align-self: flex-start; }}
  .due-box .due-label {{ font-size: 10px; text-transform: uppercase; letter-spacing: 1px; opacity: .85; }}
  .due-box .due-amt {{ font-size: 24px; font-weight: 800; margin-top: 2px; }}
  .due-box .due-sub {{ font-size: 10.5px; opacity: .85; margin-top: 2px; }}
  .etims {{ display: inline-block; margin-top: 16px; padding: 5px 11px; border: 1px solid #bbf7d0; background: #f0fdf4; color: #15803d; font-size: 11px; font-weight: 600; border-radius: 6px; }}

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
  table.tot td.paid {{ color: #16a34a; }}
  table.tot tr.total td {{ border-top: 1px solid #e5e7eb; font-weight: 700; font-size: 13px; color: #111827; padding-top: 9px; }}
  table.tot tr.balance td {{ background: #f9fafb; border-top: 2px solid {accent}; font-weight: 800; font-size: 14px; color: {accent}; padding: 10px; }}

  /* Notes + footer */
  .notes {{ margin-top: 30px; padding: 14px 16px; background: #f9fafb; border-left: 3px solid {accent}; border-radius: 0 6px 6px 0; }}
  .notes h4 {{ margin: 0 0 4px; font-size: 9.5px; font-weight: 700; text-transform: uppercase; letter-spacing: .6px; color: #9ca3af; }}
  .notes p {{ margin: 0; font-size: 12px; color: #374151; white-space: pre-line; }}
  .foot {{ margin-top: 36px; padding-top: 14px; border-top: 1px solid #e5e7eb; text-align: center; color: #9ca3af; font-size: 10.5px; }}
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
      <div class="doc-title">{title}</div>
      <table class="meta">
        <tr><td class="k">{number_label}</td><td class="v">{number}</td></tr>
        <tr><td class="k">Issue Date</td><td class="v">{issue}</td></tr>
        <tr><td class="k">{date2_label}</td><td class="v">{due}</td></tr>
        <tr><td class="k">Currency</td><td class="v">{currency}</td></tr>
      </table>
    </div>
  </div>

  <div class="info">
    <div class="billto">
      <div class="label">Bill To</div>
      <div class="name">{customer}</div>
      {cust_addr}
      {cust_pin}
      {etims}
    </div>
    <div class="due-box">
      <div class="due-label">{summary_label}</div>
      <div class="due-amt">{hero_amount}</div>
      <div class="due-sub">{date2_label} {due}</div>
    </div>
  </div>

  <table class="items">
    <thead><tr><th>Description</th><th class="num">Qty</th><th class="num">Unit Price</th><th class="num">VAT</th><th class="num">Amount</th></tr></thead>
    <tbody>{rows}</tbody>
  </table>

  <div class="totals">
    <table class="tot">
      {total_rows}
    </table>
  </div>

  {notes}
  <div class="foot">{footer}</div>
</div></body></html>"#,
        accent = accent,
        logo = logo,
        org = esc(&doc.org_name),
        org_meta = org_meta,
        org_contact = org_contact,
        title = esc(&doc.title),
        number = esc(&doc.number),
        customer = esc(&doc.customer_name),
        cust_addr = cust_addr,
        cust_pin = cust_pin,
        issue = esc(&doc.issue_date),
        due = esc(&doc.due_date),
        currency = esc(&doc.currency),
        etims = etims,
        rows = rows,
        total_rows = total_rows,
        number_label = esc(&doc.number_label),
        date2_label = esc(&doc.date2_label),
        summary_label = esc(&doc.summary_label),
        hero_amount = money(cur, if doc.show_payments { doc.balance_due } else { doc.gross_total }),
        notes = notes,
        footer = footer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample() -> InvoiceDocument {
        InvoiceDocument {
            org_name: "Craig's Design".into(),
            org_kra_pin: Some("P051234567X".into()),
            org_vat_number: None,
            org_address: Some("Nairobi".into()),
            org_email: Some("hi@craig.co".into()),
            org_phone: None,
            logo_url: None,
            primary_color: "#1a56db".into(),
            footer_text: Some("Thank you".into()),
            title: "TAX INVOICE".into(),
            number: "INV-2026-0001".into(),
            issue_date: "2026-06-01".into(),
            due_date: "2026-07-01".into(),
            currency: "KES".into(),
            etims_number: None,
            customer_name: "Mark Cho".into(),
            customer_address: Some("Mombasa".into()),
            customer_kra_pin: None,
            lines: vec![InvoiceDocLine {
                description: "Landscaping".into(),
                quantity: dec!(2),
                unit_price: dec!(1000),
                vat_amount: dec!(320),
                line_total: dec!(2000),
            }],
            subtotal: dec!(2000),
            discount_total: dec!(0),
            tax_total: dec!(320),
            gross_total: dec!(2320),
            amount_paid: dec!(0),
            balance_due: dec!(2320),
            notes: Some("Pay promptly".into()),
            number_label: "Invoice No".into(),
            date2_label: "Due Date".into(),
            summary_label: "Balance Due".into(),
            show_payments: true,
        }
    }

    #[test]
    fn renders_self_contained_html() {
        let html = render_invoice_html(&sample());
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("TAX INVOICE"));
        assert!(html.contains("INV-2026-0001"));
        assert!(html.contains("Mark Cho"));
        assert!(html.contains("Landscaping"));
        // accent applied
        assert!(html.contains("#1a56db"));
    }

    #[test]
    fn escapes_html() {
        let mut d = sample();
        d.customer_name = "A & <b>B</b>".into();
        let html = render_invoice_html(&d);
        assert!(html.contains("A &amp; &lt;b&gt;B&lt;/b&gt;"));
    }
}
