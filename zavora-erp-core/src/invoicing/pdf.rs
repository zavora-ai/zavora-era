//! Minimal, dependency-free PDF generation for invoices.
//!
//! The project carries no PDF crate, and pulling in a headless-browser/HTML→PDF
//! pipeline is heavy and environment-dependent. For a clean, portable invoice
//! attachment we hand-build a small but valid PDF 1.4 document: a single page
//! with the standard Helvetica font, drawing the invoice header, party block,
//! line-item table, totals, and the template footer. Colours come from the
//! selected template (header rule + heading colour).
//!
//! This is intentionally simple (text + rules, no logos/images), but it is a
//! real, openable PDF — verified by the leading `%PDF-1.4` header, a complete
//! xref table, and a trailer.

use rust_decimal::Decimal;

/// Everything the renderer needs to draw an invoice page. Kept free of DB types
/// so it is trivial to unit-test and reuse.
pub struct InvoicePdfData {
    pub org_name: String,
    pub invoice_number: String,
    pub invoice_type_label: String, // "Tax Invoice" / "Credit Note"
    pub issue_date: String,
    pub due_date: String,
    pub currency: String,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub lines: Vec<InvoicePdfLine>,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub notes: Option<String>,
    pub footer_text: Option<String>,
    /// Heading/accent colour as (r,g,b) in 0.0..=1.0, parsed from the template.
    pub accent_rgb: (f32, f32, f32),
}

pub struct InvoicePdfLine {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub line_total: Decimal,
}

/// Escape a string for use inside a PDF text literal `( ... )`.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            // Drop non-ASCII to stay within WinAnsi/Helvetica's safe range.
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn money(d: Decimal) -> String {
    // Two-decimal, thousands-separated.
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
    format!("{}{}.{}", if neg { "-" } else { "" }, grouped, frac)
}

/// A tiny content-stream builder that tracks the current Y cursor.
struct Canvas {
    ops: String,
    y: f32,
}

impl Canvas {
    fn new() -> Self {
        Self { ops: String::new(), y: 800.0 }
    }

    fn text(&mut self, x: f32, size: f32, s: &str) {
        self.ops.push_str(&format!(
            "BT /F1 {size} Tf 1 0 0 1 {x} {y} Tm ({txt}) Tj ET\n",
            size = size,
            x = x,
            y = self.y,
            txt = esc(s),
        ));
    }

    fn text_bold(&mut self, x: f32, size: f32, s: &str) {
        self.ops.push_str(&format!(
            "BT /F2 {size} Tf 1 0 0 1 {x} {y} Tm ({txt}) Tj ET\n",
            size = size,
            x = x,
            y = self.y,
            txt = esc(s),
        ));
    }

    fn text_color(&mut self, x: f32, size: f32, s: &str, rgb: (f32, f32, f32)) {
        self.ops.push_str(&format!("{r} {g} {b} rg\n", r = rgb.0, g = rgb.1, b = rgb.2));
        self.text_bold(x, size, s);
        self.ops.push_str("0 0 0 rg\n");
    }

    fn rule(&mut self, x0: f32, x1: f32, rgb: (f32, f32, f32)) {
        self.ops.push_str(&format!(
            "{r} {g} {b} RG 0.8 w {x0} {y} m {x1} {y} l S 0 0 0 RG\n",
            r = rgb.0, g = rgb.1, b = rgb.2, x0 = x0, x1 = x1, y = self.y,
        ));
    }

    fn down(&mut self, dy: f32) {
        self.y -= dy;
    }
}

/// Render the invoice to PDF bytes.
pub fn render_invoice_pdf(data: &InvoicePdfData) -> Vec<u8> {
    let accent = data.accent_rgb;
    let mut c = Canvas::new();

    // Header: org name (accent) + document title.
    c.text_color(50.0, 20.0, &data.org_name, accent);
    c.down(26.0);
    c.text_bold(50.0, 14.0, &data.invoice_type_label);
    c.down(6.0);
    c.rule(50.0, 545.0, accent);
    c.down(20.0);

    // Invoice meta (right-aligned-ish via fixed columns) + bill-to.
    c.text_bold(50.0, 10.0, "BILL TO");
    c.text_bold(330.0, 10.0, "INVOICE");
    c.down(14.0);
    c.text(50.0, 10.0, &data.customer_name);
    c.text(330.0, 10.0, &format!("No:    {}", data.invoice_number));
    c.down(13.0);
    if let Some(email) = &data.customer_email {
        c.text(50.0, 10.0, email);
    }
    c.text(330.0, 10.0, &format!("Date:  {}", data.issue_date));
    c.down(13.0);
    c.text(330.0, 10.0, &format!("Due:   {}", data.due_date));
    c.down(24.0);

    // Line-item table header.
    c.rule(50.0, 545.0, accent);
    c.down(14.0);
    c.text_bold(50.0, 9.0, "DESCRIPTION");
    c.text_bold(330.0, 9.0, "QTY");
    c.text_bold(390.0, 9.0, "UNIT PRICE");
    c.text_bold(490.0, 9.0, "AMOUNT");
    c.down(6.0);
    c.rule(50.0, 545.0, (0.8, 0.8, 0.8));
    c.down(16.0);

    for line in &data.lines {
        let desc = if line.description.len() > 46 {
            format!("{}…", &line.description[..45])
        } else {
            line.description.clone()
        };
        c.text(50.0, 9.0, &desc);
        c.text(330.0, 9.0, &format!("{}", line.quantity.normalize()));
        c.text(390.0, 9.0, &money(line.unit_price));
        c.text(490.0, 9.0, &money(line.line_total));
        c.down(15.0);
        if c.y < 140.0 {
            // Keep within a single page; truncate gracefully.
            c.text(50.0, 8.0, "… additional lines omitted …");
            c.down(15.0);
            break;
        }
    }

    c.down(2.0);
    c.rule(330.0, 545.0, (0.8, 0.8, 0.8));
    c.down(16.0);

    // Totals block.
    let cur = &data.currency;
    let mut total_row = |c: &mut Canvas, label: &str, val: Decimal, bold: bool| {
        if bold {
            c.text_bold(390.0, 10.0, label);
            c.text_bold(490.0, 10.0, &money(val));
        } else {
            c.text(390.0, 9.0, label);
            c.text(490.0, 9.0, &money(val));
        }
        c.down(14.0);
    };
    total_row(&mut c, &format!("Subtotal ({cur})"), data.subtotal, false);
    if data.discount_total > Decimal::ZERO {
        total_row(&mut c, "Discount", data.discount_total, false);
    }
    total_row(&mut c, "VAT", data.tax_total, false);
    total_row(&mut c, "Total", data.gross_total, true);
    if data.amount_paid > Decimal::ZERO {
        total_row(&mut c, "Paid", data.amount_paid, false);
    }
    c.text_color(390.0, 11.0, "Balance Due", accent);
    c.text_color(490.0, 11.0, &money(data.balance_due), accent);
    c.down(28.0);

    // Notes.
    if let Some(notes) = &data.notes {
        if !notes.trim().is_empty() {
            c.text_bold(50.0, 9.0, "Notes");
            c.down(13.0);
            for chunk in wrap(notes, 90) {
                c.text(50.0, 9.0, &chunk);
                c.down(12.0);
            }
        }
    }

    // Footer pinned near the bottom.
    if let Some(footer) = &data.footer_text {
        c.y = 70.0;
        c.rule(50.0, 545.0, (0.85, 0.85, 0.85));
        c.down(14.0);
        for chunk in wrap(footer, 100) {
            c.text(50.0, 8.0, &chunk);
            c.down(11.0);
        }
    }

    assemble(&c.ops)
}

/// Naive word-wrap to a max character width.
fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if cur.len() + word.len() + 1 > width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Assemble the PDF objects, xref table and trailer around a content stream.
fn assemble(content: &str) -> Vec<u8> {
    // Objects:
    // 1 Catalog, 2 Pages, 3 Page, 4 Contents, 5 Font Helvetica, 6 Font Helvetica-Bold
    let mut objects: Vec<String> = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objects.push("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string());
    objects.push(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] \
         /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> /Contents 4 0 R >>"
            .to_string(),
    );
    objects.push(format!(
        "<< /Length {} >>\nstream\n{}\nendstream",
        content.len() + 1,
        content
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_string());

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, body));
    }

    let xref_start = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for off in &offsets {
        pdf.push_str(&format!("{:010} 00000 n \n", off));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF",
        objects.len() + 1,
        xref_start
    ));

    pdf.into_bytes()
}

/// Parse a `#rrggbb` colour into (r,g,b) floats; falls back to a calm blue.
pub fn parse_hex_color(hex: &str) -> (f32, f32, f32) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        }
    }
    (0.10, 0.34, 0.86) // #1a56db-ish
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample() -> InvoicePdfData {
        InvoicePdfData {
            org_name: "Sample Co".to_string(),
            invoice_number: "INV-2026-0001".to_string(),
            invoice_type_label: "Tax Invoice".to_string(),
            issue_date: "2026-06-01".to_string(),
            due_date: "2026-07-01".to_string(),
            currency: "KES".to_string(),
            customer_name: "Mark Cho".to_string(),
            customer_email: Some("mark@example.com".to_string()),
            lines: vec![InvoicePdfLine {
                description: "Landscaping".to_string(),
                quantity: dec!(2),
                unit_price: dec!(1000),
                line_total: dec!(2000),
            }],
            subtotal: dec!(2000),
            discount_total: dec!(0),
            tax_total: dec!(320),
            gross_total: dec!(2320),
            amount_paid: dec!(0),
            balance_due: dec!(2320),
            notes: Some("Thank you".to_string()),
            footer_text: Some("Bank: 123456".to_string()),
            accent_rgb: (0.1, 0.34, 0.86),
        }
    }

    #[test]
    fn produces_valid_pdf_header_and_trailer() {
        let bytes = render_invoice_pdf(&sample());
        assert!(bytes.starts_with(b"%PDF-1.4"));
        let tail = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(8)..]);
        assert!(tail.contains("%%EOF"));
        assert!(bytes.len() > 500);
    }

    #[test]
    fn hex_parses() {
        assert_eq!(parse_hex_color("#ffffff"), (1.0, 1.0, 1.0));
        assert_eq!(parse_hex_color("000000"), (0.0, 0.0, 0.0));
    }

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money(dec!(1234567.5)), "1,234,567.50");
        assert_eq!(money(dec!(-50)), "-50.00");
    }
}
