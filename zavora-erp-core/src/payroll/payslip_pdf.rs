//! Minimal, dependency-free payslip PDF renderer (same lightweight approach as
//! `invoicing::pdf`): build a content stream and assemble a one-page PDF.

use rust_decimal::Decimal;

/// Everything needed to draw a payslip page (kept free of DB types).
pub struct PayslipPdfData {
    pub company_name: String,
    pub employee_name: String,
    pub staff_number: String,
    pub kra_pin: String,
    pub pay_date: String,
    pub period_label: String,
    /// Itemized earning lines (name, amount). Empty falls back to "Basic pay".
    pub earnings: Vec<(String, Decimal)>,
    pub gross_salary: Decimal,
    pub taxable_income: Decimal,
    pub paye: Decimal,
    pub personal_relief: Decimal,
    pub net_paye: Decimal,
    pub nssf_employee: Decimal,
    pub nssf_employer: Decimal,
    pub sha: Decimal,
    pub housing_levy_employee: Decimal,
    pub housing_levy_employer: Decimal,
    pub helb: Decimal,
    /// Voluntary/loan deduction lines (name, amount).
    pub other_deductions: Vec<(String, Decimal)>,
    pub total_deductions: Decimal,
    pub net_salary: Decimal,
    pub ytd_gross: Decimal,
    pub ytd_paye: Decimal,
    pub ytd_net: Decimal,
}

struct Canvas {
    ops: String,
    y: f32,
}
impl Canvas {
    fn new() -> Self { Self { ops: String::new(), y: 800.0 } }
    fn text(&mut self, x: f32, size: f32, s: &str) {
        self.ops.push_str(&format!("BT /F1 {size} Tf 1 0 0 1 {x} {y} Tm ({txt}) Tj ET\n", y = self.y, txt = esc(s)));
    }
    fn bold(&mut self, x: f32, size: f32, s: &str) {
        self.ops.push_str(&format!("BT /F2 {size} Tf 1 0 0 1 {x} {y} Tm ({txt}) Tj ET\n", y = self.y, txt = esc(s)));
    }
    fn rule(&mut self) {
        self.ops.push_str(&format!("0.7 0.7 0.7 RG 0.6 w 50 {y} m 545 {y} l S 0 0 0 RG\n", y = self.y));
    }
    fn down(&mut self, dy: f32) { self.y -= dy; }
    /// A label:value row (value right-aligned at x=545).
    fn row(&mut self, label: &str, val: Decimal, bold: bool) {
        let v = fmt_money(val);
        if bold { self.bold(50.0, 10.0, label); } else { self.text(60.0, 10.0, label); }
        let x = 545.0 - (v.len() as f32) * 5.6;
        if bold { self.bold(x, 10.0, &v); } else { self.text(x, 10.0, &v); }
        self.down(16.0);
    }
}

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)") }

fn fmt_money(d: Decimal) -> String {
    // "KES 12,345.67" with thousands separators.
    let d = d.round_dp(2);
    let neg = d.is_sign_negative();
    let s = d.abs().to_string();
    let (int, frac) = match s.split_once('.') { Some((i, f)) => (i.to_string(), format!("{:0<2}", f)), None => (s, "00".into()) };
    let mut out = String::new();
    for (i, ch) in int.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    let int_sep: String = out.chars().rev().collect();
    format!("{}KES {}.{}", if neg { "-" } else { "" }, int_sep, &frac[..2])
}

/// Render a payslip to PDF bytes.
pub fn render_payslip_pdf(d: &PayslipPdfData) -> Vec<u8> {
    let mut c = Canvas::new();
    c.bold(50.0, 18.0, &d.company_name);
    c.down(22.0);
    c.bold(50.0, 13.0, "Payslip");
    c.text(430.0, 10.0, &format!("Pay date: {}", d.pay_date));
    c.down(16.0);
    c.text(50.0, 10.0, &format!("Period: {}", d.period_label));
    c.down(18.0);
    c.rule();
    c.down(18.0);

    c.bold(50.0, 11.0, d.employee_name.as_str());
    c.down(15.0);
    c.text(50.0, 10.0, &format!("Staff No: {}", d.staff_number));
    c.down(14.0);
    c.text(50.0, 10.0, &format!("KRA PIN: {}", d.kra_pin));
    c.down(22.0);

    c.bold(50.0, 11.0, "Earnings");
    c.down(18.0);
    if d.earnings.is_empty() {
        c.row("Basic pay", d.gross_salary, false);
    } else {
        for (name, amt) in &d.earnings { c.row(name, *amt, false); }
    }
    c.rule();
    c.down(16.0);
    c.row("Gross pay", d.gross_salary, true);
    c.down(6.0);

    c.bold(50.0, 11.0, "Statutory deductions");
    c.down(18.0);
    c.row("PAYE (after relief)", d.net_paye, false);
    c.row("NSSF", d.nssf_employee, false);
    c.row("SHA", d.sha, false);
    c.row("Housing Levy", d.housing_levy_employee, false);
    if d.helb > Decimal::ZERO { c.row("HELB", d.helb, false); }

    if !d.other_deductions.is_empty() {
        c.down(4.0);
        c.bold(50.0, 11.0, "Other deductions");
        c.down(18.0);
        for (name, amt) in &d.other_deductions { c.row(name, *amt, false); }
    }

    c.down(4.0);
    c.rule();
    c.down(16.0);
    c.row("Total deductions", d.total_deductions, true);
    c.down(4.0);
    c.rule();
    c.down(20.0);
    c.bold(50.0, 12.0, "NET PAY");
    let v = fmt_money(d.net_salary);
    c.bold(545.0 - (v.len() as f32) * 6.6, 12.0, &v);
    c.down(28.0);

    c.bold(50.0, 11.0, "Employer contributions");
    c.down(18.0);
    c.row("NSSF (employer)", d.nssf_employer, false);
    c.row("Housing Levy (employer)", d.housing_levy_employer, false);
    c.down(6.0);

    c.bold(50.0, 11.0, "Year to date");
    c.down(18.0);
    c.row("Gross", d.ytd_gross, false);
    c.row("PAYE", d.ytd_paye, false);
    c.row("Net pay", d.ytd_net, false);
    c.down(12.0);
    c.text(50.0, 8.0, "PAYE shown after personal relief. Figures in KES. System-generated payslip.");

    assemble(&c.ops)
}

fn assemble(content: &str) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objects.push("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string());
    objects.push("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> /Contents 4 0 R >>".to_string());
    objects.push(format!("<< /Length {} >>\nstream\n{}\nendstream", content.len() + 1, content));
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
    for off in &offsets { pdf.push_str(&format!("{:010} 00000 n \n", off)); }
    pdf.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF", objects.len() + 1, xref_start));
    pdf.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    #[test]
    fn renders_a_pdf() {
        let d = PayslipPdfData {
            company_name: "Zavora Technologies Ltd".into(), employee_name: "Grace W".into(),
            staff_number: "E-01".into(), kra_pin: "A00X".into(), pay_date: "2025-12-31".into(),
            period_label: "December 2025".into(),
            earnings: vec![("Basic Pay".into(), dec!(80000)), ("Housing".into(), dec!(20000))],
            gross_salary: dec!(100000), taxable_income: dec!(100000),
            paye: dec!(23685), personal_relief: dec!(2400), net_paye: dec!(21285), nssf_employee: dec!(2160),
            nssf_employer: dec!(2160), sha: dec!(2750), housing_levy_employee: dec!(1500), housing_levy_employer: dec!(1500),
            helb: dec!(0), other_deductions: vec![("SACCO".into(), dec!(3000))], total_deductions: dec!(27695),
            net_salary: dec!(72305), ytd_gross: dec!(100000), ytd_paye: dec!(21285), ytd_net: dec!(72305),
        };
        let bytes = render_payslip_pdf(&d);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.len() > 500);
    }
    #[test]
    fn money_format() {
        assert_eq!(fmt_money(dec!(72304.65)), "KES 72,304.65");
        assert_eq!(fmt_money(dec!(1500)), "KES 1,500.00");
    }
}
