//! Spreadsheet (`.xlsx` / `.xls` / `.ods`) bank-statement import.
//!
//! Kenyan M-Pesa **full statements** and several bank exports are distributed as
//! Excel workbooks with a clean tabular layout — explicit *Paid in* / *Withdrawn*
//! (or *Debit* / *Credit*) columns and a running *Balance*. Unlike a PDF text
//! layer, the columns are unambiguous, so we map them by **header name** and do
//! not need the running-balance reconciliation the PDF parser relies on (M-Pesa
//! balances even reset to zero on overdraft repayment, which would defeat it).
//!
//! Output is the same [`ParsedPdfRow`] the PDF flow produces, so both feed the
//! identical review → confirm → deterministic-CSV-import pipeline. Nothing is
//! committed here; every row is shown for human review first.

use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, DataType, Reader};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::str::FromStr;

use super::statement_pdf::ParsedPdfRow;

/// Parse the first sheet that has a recognisable transaction header into rows.
/// Returns an empty vec when the workbook can't be read or no statement table is
/// found (the caller then reports "nothing detected").
///
/// M-Pesa (and several bank) workbooks lead with a **Summary** sheet of
/// aggregates (Pay Bill / Buy Goods / … totals) before the detailed transaction
/// sheet. That summary must never be mistaken for the statement, so summary-like
/// sheets are skipped outright and the detailed transaction sheet is preferred.
pub fn parse_statement_xlsx(bytes: &[u8]) -> Vec<ParsedPdfRow> {
    parse_statement_xlsx_checked(bytes).0
}

/// Reconciliation of the detailed rows against the workbook's Summary totals.
/// `None` when the workbook has no usable Summary sheet to check against.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatementReconciliation {
    /// Total credits (money in) from the Summary sheet.
    pub summary_paid_in: Decimal,
    /// Total debits (money out) from the Summary sheet.
    pub summary_paid_out: Decimal,
    /// Total credits summed from the detailed transaction rows.
    pub detail_paid_in: Decimal,
    /// Total debits summed from the detailed transaction rows.
    pub detail_paid_out: Decimal,
    /// True when both sides reconcile to the cent.
    pub balanced: bool,
}

/// Parse the detailed rows **and** reconcile them against the Summary sheet.
///
/// The Summary sheet is never parsed as transactions, but its "Total" Paid In /
/// Paid Out figures are an authoritative checksum: if the sum of the detailed
/// rows doesn't match, the extraction is wrong (a missed/duplicated row, a
/// column mis-map) and the caller should warn before importing.
pub fn parse_statement_xlsx_checked(
    bytes: &[u8],
) -> (Vec<ParsedPdfRow>, Option<StatementReconciliation>) {
    let cursor = Cursor::new(bytes.to_vec());
    let mut wb = match open_workbook_auto_from_rs(cursor) {
        Ok(w) => w,
        Err(_) => return (Vec::new(), None),
    };

    // Read the Summary totals first (if a summary sheet exists).
    let mut summary: Option<(Decimal, Decimal)> = None;
    for name in wb.sheet_names().to_owned() {
        if is_summary_sheet(&name) {
            if let Ok(range) = wb.worksheet_range(&name) {
                summary = summary_totals(&range);
            }
            break;
        }
    }

    // Parse the first non-summary sheet that yields transactions.
    let mut rows = Vec::new();
    for name in wb.sheet_names().to_owned() {
        if is_summary_sheet(&name) {
            continue;
        }
        if let Ok(range) = wb.worksheet_range(&name) {
            let parsed = parse_range(&range);
            if !parsed.is_empty() {
                rows = parsed;
                break;
            }
        }
    }

    let recon = summary.map(|(paid_in, paid_out)| {
        let detail_in: Decimal = rows.iter().filter_map(|r| r.credit).sum();
        let detail_out: Decimal = rows.iter().filter_map(|r| r.debit).sum();
        let cent = Decimal::new(1, 2);
        StatementReconciliation {
            summary_paid_in: paid_in,
            summary_paid_out: paid_out,
            detail_paid_in: detail_in,
            detail_paid_out: detail_out,
            balanced: (detail_in - paid_in).abs() <= cent && (detail_out - paid_out).abs() <= cent,
        }
    });

    (rows, recon)
}

/// Extract the "Total" Paid In / Paid Out figures from a Summary sheet. The
/// M-Pesa summary is `Transaction Type | Paid In | Paid Out` with a final
/// `Total` row. Returns `(paid_in, paid_out)` from that total row, else `None`.
fn summary_totals(range: &calamine::Range<Data>) -> Option<(Decimal, Decimal)> {
    // Locate the Paid In / Paid Out columns from the header.
    let rows: Vec<&[Data]> = range.rows().collect();
    let mut paid_in_col = None;
    let mut paid_out_col = None;
    for row in rows.iter().take(10) {
        for (i, cell) in row.iter().enumerate() {
            let h = cell_string(Some(cell)).to_lowercase();
            if h.contains("paid in") || h.contains("money in") {
                paid_in_col = Some(i);
            } else if h.contains("paid out") || h.contains("money out") {
                paid_out_col = Some(i);
            }
        }
        if paid_in_col.is_some() && paid_out_col.is_some() {
            break;
        }
    }
    let (pin, pout) = (paid_in_col?, paid_out_col?);
    // Prefer an explicit "Total" row; else sum every numeric data row.
    for row in &rows {
        let label = row.first().map(|c| cell_string(Some(c)).to_lowercase()).unwrap_or_default();
        if label == "total" {
            let i = cell_decimal(row.get(pin)).unwrap_or(Decimal::ZERO);
            let o = cell_decimal(row.get(pout)).unwrap_or(Decimal::ZERO);
            return Some((i, o));
        }
    }
    None
}

/// True for sheets that hold aggregates/metadata rather than the transaction
/// ledger — these must be skipped. Matches the M-Pesa "Summary" tab and common
/// bank equivalents ("Totals", "Overview").
fn is_summary_sheet(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    n == "summary"
        || n.starts_with("summary")
        || n.contains("summary")
        || n == "totals"
        || n == "overview"
}

/// Column roles resolved from a header row.
#[derive(Default)]
struct Cols {
    date: Option<usize>,
    desc: Option<usize>,
    other_party: Option<usize>,
    credit: Option<usize>,
    debit: Option<usize>,
    amount: Option<usize>,
    balance: Option<usize>,
}

fn parse_range(range: &calamine::Range<Data>) -> Vec<ParsedPdfRow> {
    let rows: Vec<&[Data]> = range.rows().collect();
    // Find the header row: the first row that names a balance column together
    // with at least one money-movement column.
    let mut header_idx = None;
    let mut cols = Cols::default();
    for (i, row) in rows.iter().enumerate().take(40) {
        let c = map_columns(row);
        let has_movement = c.credit.is_some() || c.debit.is_some() || c.amount.is_some();
        if c.balance.is_some() && has_movement && c.date.is_some() {
            header_idx = Some(i);
            cols = c;
            break;
        }
    }
    let Some(start) = header_idx else { return Vec::new() };

    let mut out = Vec::new();
    for row in rows.iter().skip(start + 1) {
        let Some(date) = cols.date.and_then(|i| cell_date(row.get(i))) else { continue };

        let mut credit = cols.credit.and_then(|i| cell_decimal(row.get(i))).filter(|d| *d != Decimal::ZERO);
        let mut debit = cols.debit.and_then(|i| cell_decimal(row.get(i))).filter(|d| *d != Decimal::ZERO);

        // Single signed amount column (negative = money out).
        if credit.is_none() && debit.is_none() {
            if let Some(amt) = cols.amount.and_then(|i| cell_decimal(row.get(i))) {
                if amt < Decimal::ZERO {
                    debit = Some(amt.abs());
                } else if amt > Decimal::ZERO {
                    credit = Some(amt);
                }
            }
        }
        if credit.is_none() && debit.is_none() {
            continue; // zero/blank movement → not a transaction
        }

        let mut description = cols.desc.map(|i| cell_string(row.get(i))).unwrap_or_default();
        if let Some(op) = cols.other_party.map(|i| cell_string(row.get(i))) {
            if !op.is_empty() && !description.contains(&op) {
                description = if description.is_empty() { op } else { format!("{description} — {op}") };
            }
        }

        out.push(ParsedPdfRow {
            value_date: date,
            description: description.trim().to_string(),
            debit: debit.map(|d| d.abs()),
            credit: credit.map(|c| c.abs()),
            balance: cols.balance.and_then(|i| cell_decimal(row.get(i))),
            confidence: 0.95, // structured spreadsheet columns — unambiguous
        });
    }
    out
}

/// Resolve column roles from a header row by matching header text keywords.
fn map_columns(row: &[Data]) -> Cols {
    let mut c = Cols::default();
    for (i, cell) in row.iter().enumerate() {
        let h = cell_string(Some(cell)).to_lowercase();
        let h = h.trim();
        if h.is_empty() {
            continue;
        }
        // Order matters: check the more specific labels first.
        if c.date.is_none() && (h.contains("completion time") || h.contains("value date") || h.contains("transaction date") || h == "date" || h.contains("date")) {
            c.date = Some(i);
        } else if c.credit.is_none() && (h.contains("paid in") || h.contains("money in") || h == "credit" || h.contains("deposit")) {
            c.credit = Some(i);
        } else if c.debit.is_none() && (h.contains("withdrawn") || h.contains("money out") || h == "debit" || h.contains("withdrawal")) {
            c.debit = Some(i);
        } else if c.balance.is_none() && h.contains("balance") {
            c.balance = Some(i);
        } else if c.amount.is_none() && h == "amount" {
            c.amount = Some(i);
        } else if c.desc.is_none() && (h.contains("details") || h.contains("description") || h.contains("narrative") || h.contains("particulars")) {
            c.desc = Some(i);
        } else if c.other_party.is_none() && h.contains("other party") {
            c.other_party = Some(i);
        }
    }
    c
}

fn cell_string(cell: Option<&Data>) -> String {
    match cell {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Empty) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn cell_decimal(cell: Option<&Data>) -> Option<Decimal> {
    match cell {
        Some(Data::Int(i)) => Some(Decimal::from(*i)),
        Some(Data::Float(f)) => Decimal::from_str(&format!("{f}")).ok().map(|d| d.round_dp(2)),
        Some(Data::String(s)) => {
            let cleaned: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
            if cleaned.is_empty() { None } else { Decimal::from_str(&cleaned).ok() }
        }
        _ => None,
    }
}

fn cell_date(cell: Option<&Data>) -> Option<NaiveDate> {
    match cell {
        Some(d @ Data::DateTime(_)) => d.as_datetime().map(|dt| dt.date()),
        Some(Data::DateTimeIso(s)) | Some(Data::String(s)) => parse_date_str(s),
        _ => None,
    }
}

/// Parse the date out of a free-form cell string: `2025-12-19 09:32:46`,
/// `2025-12-19`, `19/12/2025`, or `19 Dec 2025`.
fn parse_date_str(s: &str) -> Option<NaiveDate> {
    let first = s.split_whitespace().next().unwrap_or(s).trim();
    for fmt in ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%d-%m-%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(first, fmt) {
            return Some(d);
        }
    }
    // `19 Dec 2025` / `19 December 2025` (three tokens).
    let toks: Vec<&str> = s.split_whitespace().collect();
    if toks.len() >= 3 {
        let joined = format!("{} {} {}", toks[0], toks[1], toks[2]);
        for fmt in ["%d %b %Y", "%d %B %Y"] {
            if let Ok(d) = NaiveDate::parse_from_str(&joined, fmt) {
                return Some(d);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parse_date_str_handles_mpesa_datetime() {
        assert_eq!(parse_date_str("2025-12-19 09:32:46"), NaiveDate::from_ymd_opt(2025, 12, 19));
        assert_eq!(parse_date_str("19/12/2025"), NaiveDate::from_ymd_opt(2025, 12, 19));
        assert_eq!(parse_date_str("19 Dec 2025"), NaiveDate::from_ymd_opt(2025, 12, 19));
    }

    #[test]
    fn maps_mpesa_columns_and_reads_rows() {
        use calamine::{Data, Range};
        // Reproduce the M-Pesa merchant sheet shape (header on row 8).
        let mut r: Range<Data> = Range::new((0, 0), (10, 8));
        let hdr = [
            "Receipt No", "Completion Time", "Details", "Transaction Status",
            "Paid in", "Withdrawn", "Balance", "Transaction Type", "Other Party",
        ];
        for (j, h) in hdr.iter().enumerate() {
            r.set_value((8, j as u32), Data::String(h.to_string()));
        }
        // A "Paid in" credit row and a "Withdrawn" debit row.
        let credit = [
            Data::String("TFS7NM7ZBT".into()), Data::String("2025-06-28 02:04:33".into()),
            Data::String("Merchant Payment from X".into()), Data::String("Completed".into()),
            Data::Float(150.0), Data::Float(0.0), Data::Float(150.0),
            Data::String("Customer Merchant Payment".into()), Data::String("7480407-ZAVORA HQ".into()),
        ];
        let debit = [
            Data::String("TFS7NMPWRN".into()), Data::String("2025-06-28 02:36:54".into()),
            Data::String("Biashara Overdraft Repayment".into()), Data::String("Completed".into()),
            Data::Float(0.0), Data::Float(150.0), Data::Float(0.0),
            Data::String("OD Payment Transfer".into()), Data::String("804080-Boost Biashara".into()),
        ];
        for (j, v) in credit.iter().enumerate() { r.set_value((9, j as u32), v.clone()); }
        for (j, v) in debit.iter().enumerate() { r.set_value((10, j as u32), v.clone()); }

        let rows = parse_range(&r);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].credit, Some(dec!(150.00)));
        assert_eq!(rows[0].debit, None);
        assert_eq!(rows[0].balance, Some(dec!(150.00)));
        assert_eq!(rows[0].value_date, NaiveDate::from_ymd_opt(2025, 6, 28).unwrap());
        assert!(rows[0].description.contains("Merchant Payment"));
        assert!(rows[0].description.contains("ZAVORA HQ"), "other party appended");
        assert_eq!(rows[1].debit, Some(dec!(150.00)));
        assert_eq!(rows[1].credit, None);
    }

    #[test]
    fn skips_summary_sheet_by_name() {
        assert!(is_summary_sheet("Summary"));
        assert!(is_summary_sheet("summary"));
        assert!(is_summary_sheet("Account Summary"));
        assert!(is_summary_sheet("Totals"));
        assert!(!is_summary_sheet("M-PESA FULL STATEMENT - Merchan"));
        assert!(!is_summary_sheet("Transactions"));
    }

    #[test]
    fn summary_totals_reads_mpesa_total_row() {
        use calamine::{Data, Range};
        let mut r: Range<Data> = Range::new((0, 0), (9, 2));
        r.set_value((0, 0), Data::String("Transaction Type".into()));
        r.set_value((0, 1), Data::String("Paid In".into()));
        r.set_value((0, 2), Data::String("Paid Out".into()));
        r.set_value((2, 0), Data::String("Buy Goods".into()));
        r.set_value((2, 1), Data::Float(55150.0));
        r.set_value((2, 2), Data::Float(0.0));
        r.set_value((9, 0), Data::String("Total".into()));
        r.set_value((9, 1), Data::Float(55150.0));
        r.set_value((9, 2), Data::Float(55150.0));
        let (pin, pout) = summary_totals(&r).expect("totals");
        assert_eq!(pin, dec!(55150));
        assert_eq!(pout, dec!(55150));
    }
}
