//! Heuristic parser for **bank-statement text** extracted from a PDF (digital
//! text layer or OCR). This is the fallback "generic" parser for the PDF import
//! flow: it turns free-form statement text into candidate transaction rows that
//! the user MUST review and edit before they are committed through the normal
//! deterministic CSV import pipeline.
//!
//! It deliberately makes no attempt to be perfect across every bank layout —
//! per-bank templates handle the common banks precisely; this covers the long
//! tail and always defers the final say to the human review step. Every row
//! carries a `confidence` so the UI can flag uncertain extractions.
//!
//! Heuristic model: a transaction row **starts with a date token**; the money
//! amounts are the numeric tokens near the end of the line. With three trailing
//! amounts we read them as (debit, credit, balance); with two as (amount,
//! balance) using a money-in/out keyword or sign; with one as a signed amount.
//! Everything between the date and the first amount is the description.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// One candidate row parsed from statement text, with a confidence in `[0,1]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedPdfRow {
    pub value_date: NaiveDate,
    pub description: String,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub balance: Option<Decimal>,
    /// Extraction confidence in `[0,1]`; lower means the row needs closer review.
    pub confidence: f32,
}

/// Parse statement text into candidate rows. Never panics; returns an empty vec
/// when nothing date-anchored is found (the UI then shows "nothing detected").
///
/// Routes to a dedicated M-Pesa parser when the text looks like an M-Pesa
/// statement (clean, row-per-transaction), otherwise uses the generic
/// date-anchored line parser for the long tail of bank layouts.
pub fn parse_statement_text(text: &str) -> Vec<ParsedPdfRow> {
    if looks_like_mpesa(text) {
        let rows = parse_mpesa(text);
        if !rows.is_empty() {
            return rows;
        }
        // fall through to generic if the M-Pesa shape didn't yield rows
    }
    // Generic layout. Modern bank PDFs (e.g. Equity) extract one transaction per
    // line as `<ref> <date> <amount> <balance>` with the human-readable narrative
    // ("VISA PAYMENT…") on the *preceding* lines. We therefore buffer non-txn
    // narrative lines and attach them as the description of the next txn row.
    let mut rows = Vec::new();
    let mut desc_buffer: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match parse_line(line) {
            Some(mut row) => {
                if !desc_buffer.is_empty() {
                    let narrative = desc_buffer.join(" ");
                    row.description = if row.description.is_empty() {
                        narrative
                    } else {
                        format!("{narrative} {}", row.description)
                    };
                }
                desc_buffer.clear();
                rows.push(row);
            }
            None => {
                // Buffer this as narrative for the next transaction, but only if it
                // looks like a description (has letters) and is not a date-bearing
                // header/opening-balance line or column header. Cap the buffer so a
                // page of furniture can't bloat one description.
                if is_narrative_line(line) {
                    desc_buffer.push(line.to_string());
                    if desc_buffer.len() > 4 {
                        desc_buffer.remove(0);
                    }
                } else {
                    desc_buffer.clear();
                }
            }
        }
    }

    // Cross-check every row against the running balance column: the sign of
    // (balanceᵢ − balanceᵢ₋₁) is the ground truth for debit vs credit, which the
    // column-flattened text layer cannot otherwise recover.
    reconcile_running_balance(&mut rows);
    rows
}

/// True when a line is human-readable narrative worth keeping as a description
/// (has letters, is not a column header, and carries no date — date-bearing
/// lines are either transactions or statement furniture handled elsewhere).
fn is_narrative_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.contains("transaction details") || lower.contains("value date") || lower.contains("opening balance") || lower.contains("balance b/f") {
        return false;
    }
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if find_date(&tokens).is_some() {
        return false;
    }
    line.chars().any(|c| c.is_alphabetic())
}

/// Reconcile parsed rows against the running balance. For each row whose balance
/// and the previous row's balance are known, the delta determines the side:
///   * |delta| ≈ parsed amount  → high confidence; place the amount on the side
///     the delta sign dictates (credit when balance rose, debit when it fell).
///   * delta ≠ 0 but magnitude differs (a dropped/interleaved row) → trust the
///     *sign* for the side, keep the parsed amount, lower confidence for review.
///   * amount missing but delta known → recover the amount from the delta.
/// Rows without a usable balance keep their heuristic guess. The first balanced
/// row has no predecessor, so its side stays as parsed (flagged for review).
fn reconcile_running_balance(rows: &mut [ParsedPdfRow]) {
    let tol = Decimal::new(5, 2); // 0.05 absolute tolerance
    let mut prev_balance: Option<Decimal> = None;
    for row in rows.iter_mut() {
        let Some(balance) = row.balance else {
            // No balance to chain through; leave heuristic result untouched.
            continue;
        };
        if let Some(prev) = prev_balance {
            let delta = balance - prev;
            let amount = row.debit.or(row.credit);
            match amount {
                Some(amt) if (delta.abs() - amt).abs() <= tol => {
                    // Clean reconciliation: side from sign, amount confirmed.
                    if delta >= Decimal::ZERO {
                        row.credit = Some(amt);
                        row.debit = None;
                    } else {
                        row.debit = Some(amt);
                        row.credit = None;
                    }
                    row.confidence = 0.97;
                }
                Some(amt) if delta != Decimal::ZERO => {
                    // Direction is trustworthy even if magnitude disagrees (a row
                    // was likely missed); keep the parsed amount, flag for review.
                    if delta > Decimal::ZERO {
                        row.credit = Some(amt);
                        row.debit = None;
                    } else {
                        row.debit = Some(amt);
                        row.credit = None;
                    }
                    row.confidence = 0.5;
                }
                None if delta != Decimal::ZERO => {
                    // Recover a missing amount from the balance movement.
                    let amt = delta.abs();
                    if delta > Decimal::ZERO {
                        row.credit = Some(amt);
                    } else {
                        row.debit = Some(amt);
                    }
                    row.confidence = 0.8;
                }
                _ => {}
            }
        }
        // First balanced row has no predecessor → its side stays as parsed.
        prev_balance = Some(balance);
    }
}

/// True when the text carries the M-Pesa statement column header.
fn looks_like_mpesa(text: &str) -> bool {
    let lower = text.to_lowercase();
    (lower.contains("m-pesa") || lower.contains("mpesa") || lower.contains("receipt no"))
        && lower.contains("paid in")
        && lower.contains("withdrawn")
        && lower.contains("balance")
}

/// Parse an M-Pesa full/merchant statement. Each transaction row has the shape:
///   `<Receipt> <YYYY-MM-DD HH:MM:SS> <Details…> <Status> <PaidIn> <Withdrawn> <Balance> [Type] [Other Party]`
/// The three money columns (Paid in / Withdrawn / Balance) sit in the middle, so
/// we anchor on the datetime, take the **first three** money tokens that follow
/// the status word as paid-in/withdrawn/balance, and treat the text between the
/// datetime and those amounts as the description.
fn parse_mpesa(text: &str) -> Vec<ParsedPdfRow> {
    let mut rows = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 5 {
            continue;
        }
        // Find an M-Pesa datetime: a `YYYY-MM-DD` token optionally followed by a
        // `HH:MM:SS` token. The receipt code precedes it.
        let mut date_idx = None;
        for (i, t) in tokens.iter().enumerate().take(4) {
            if NaiveDate::parse_from_str(t, "%Y-%m-%d").is_ok() {
                date_idx = Some(i);
                break;
            }
        }
        let Some(di) = date_idx else { continue };
        let date = NaiveDate::parse_from_str(tokens[di], "%Y-%m-%d").ok();
        let Some(date) = date else { continue };

        // The amounts are the LAST money-looking tokens; M-Pesa puts Type / Other
        // Party text after Balance, so scan for the last run of 3 numerics that
        // are the paid-in/withdrawn/balance triple. We collect all money tokens
        // with their positions, then take the first three at/after the status.
        let money: Vec<(usize, Decimal)> = tokens
            .iter()
            .enumerate()
            .filter_map(|(i, t)| parse_money(t).map(|m| (i, m)))
            .collect();
        if money.len() < 3 {
            continue;
        }
        // Paid in / Withdrawn / Balance are three consecutive money tokens. Find
        // the first index where three consecutive token positions are all money.
        let mut triple: Option<(usize, Decimal, Decimal, Decimal)> = None;
        for w in money.windows(3) {
            if w[1].0 == w[0].0 + 1 && w[2].0 == w[1].0 + 1 {
                triple = Some((w[0].0, w[0].1, w[1].1, w[2].1));
                break;
            }
        }
        let Some((first_amt_idx, paid_in, withdrawn, balance)) = triple else { continue };

        // Description = tokens between the datetime (+ optional time) and the
        // first amount, dropping a trailing "Completed"/status word.
        let time_offset = if di + 1 < tokens.len() && tokens[di + 1].contains(':') { 2 } else { 1 };
        let desc_start = di + time_offset;
        let mut desc_end = first_amt_idx;
        // Drop a trailing status token (Completed/Failed/Pending) from description.
        if desc_end > desc_start {
            let last = tokens[desc_end - 1].to_lowercase();
            if last == "completed" || last == "failed" || last == "pending" {
                desc_end -= 1;
            }
        }
        let description = if desc_end > desc_start { tokens[desc_start..desc_end].join(" ") } else { String::new() };

        let debit = (withdrawn != Decimal::ZERO).then_some(withdrawn);
        let credit = (paid_in != Decimal::ZERO).then_some(paid_in);
        if debit.is_none() && credit.is_none() {
            continue; // zero-value row, skip
        }

        rows.push(ParsedPdfRow {
            value_date: date,
            description,
            debit,
            credit,
            balance: Some(balance),
            confidence: 0.92, // clean structured M-Pesa data
        });
    }
    rows
}

/// Try to parse a single line into a transaction row. The line must contain a
/// date — at the start (classic `Date Description … amounts` layout) or after a
/// leading reference (`<ref> <date> <amount> <balance>`, common in PDF text
/// layers). Returns `None` for header/furniture lines with no date+amounts.
fn parse_line(line: &str) -> Option<ParsedPdfRow> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }

    // Anchor on the first date anywhere in the line. Tokens before it (e.g. a
    // payment reference) become part of the description.
    let (date, date_start, date_len) = find_date(&tokens)?;
    let date_end = date_start + date_len;

    // Collect trailing money tokens (scan from the end while tokens look like money).
    let mut money_rev: Vec<Decimal> = Vec::new();
    let mut idx = tokens.len();
    while idx > date_end {
        match parse_money(tokens[idx - 1]) {
            Some(m) => {
                money_rev.push(m);
                idx -= 1;
            }
            None => break,
        }
    }
    money_rev.reverse();
    let amounts = money_rev;

    // Description = leading reference tokens (before the date) plus any tokens
    // between the date and the first trailing amount.
    let mut desc_tokens: Vec<&str> = tokens[..date_start].to_vec();
    desc_tokens.extend_from_slice(&tokens[date_end..idx]);
    let lower = line.to_lowercase();
    let looks_credit = lower.contains("credit") || lower.contains("deposit")
        || lower.contains("received") || lower.contains("c/r") || lower.contains(" cr");
    let looks_debit = lower.contains("debit") || lower.contains("withdraw")
        || lower.contains("charge") || lower.contains(" dr") || lower.contains("d/r");

    let (debit, credit, balance, conf) = match amounts.len() {
        // Date Description Debit Credit Balance  → most common Kenyan layout.
        3 => {
            let (d, c, b) = (amounts[0], amounts[1], amounts[2]);
            // A zero/blank column distinguishes debit vs credit; many PDFs print
            // 0.00 in the unused column.
            let debit = (d != Decimal::ZERO).then_some(d);
            let credit = (c != Decimal::ZERO).then_some(c);
            (debit, credit, Some(b), 0.8_f32)
        }
        // Date Description Amount Balance → use keyword/sign to assign side.
        2 => {
            let (amt, bal) = (amounts[0], amounts[1]);
            let (debit, credit) = assign_side(amt, looks_debit, looks_credit);
            (debit, credit, Some(bal), 0.6_f32)
        }
        // Date Description Amount → signed single amount, no running balance.
        1 => {
            let amt = amounts[0];
            let (debit, credit) = assign_side(amt, looks_debit, looks_credit);
            (debit, credit, None, 0.45_f32)
        }
        _ => return None, // no amounts → not a transaction row
    };

    // A row with neither debit nor credit is not usable.
    if debit.is_none() && credit.is_none() {
        return None;
    }

    let description = desc_tokens.join(" ").trim().to_string();
    // Slightly lower confidence when the description is empty (likely a mis-split).
    let confidence = if description.is_empty() { (conf - 0.1_f32).max(0.1_f32) } else { conf };

    Some(ParsedPdfRow {
        value_date: date,
        description,
        debit: debit.map(|d| d.abs()),
        credit: credit.map(|c| c.abs()),
        balance,
        confidence,
    })
}

/// Assign a single amount to debit or credit using keyword hints, then sign.
/// Defaults to debit (money out) when nothing else is decisive — a conservative
/// choice the reviewer can flip.
fn assign_side(amount: Decimal, looks_debit: bool, looks_credit: bool) -> (Option<Decimal>, Option<Decimal>) {
    if looks_credit && !looks_debit {
        return (None, Some(amount.abs()));
    }
    if looks_debit && !looks_credit {
        return (Some(amount.abs()), None);
    }
    if amount < Decimal::ZERO {
        (Some(amount.abs()), None)
    } else {
        // Positive with no hint → assume credit (money in) only when a credit
        // keyword exists; otherwise treat as debit. Here, no hint → debit.
        (Some(amount.abs()), None)
    }
}

/// Find the first date anywhere in the token list. Returns the date, the index
/// of its first token, and how many tokens it spans (1 for numeric forms, 3 for
/// `DD Mon YYYY`). Used to anchor a transaction line whose date is preceded by a
/// reference token, while still handling the classic date-first layout.
fn find_date(tokens: &[&str]) -> Option<(NaiveDate, usize, usize)> {
    for start in 0..tokens.len() {
        if let Some((d, len)) = parse_leading_date(&tokens[start..]) {
            return Some((d, start, len));
        }
    }
    None
}

/// Parse a leading date from the token list. Supports `DD/MM/YYYY`,
/// `YYYY-MM-DD`, `DD-MM-YYYY`, `DD.MM.YYYY` (single token) and `DD Mon YYYY`
/// (three tokens). Returns the date and how many tokens it consumed.
fn parse_leading_date(tokens: &[&str]) -> Option<(NaiveDate, usize)> {
    let t0 = tokens[0];
    // Single-token numeric forms.
    for fmt in ["%d/%m/%Y", "%Y-%m-%d", "%d-%m-%Y", "%d.%m.%Y", "%m/%d/%Y", "%d/%m/%y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t0, fmt) {
            return Some((d, 1));
        }
    }
    // Three-token "DD Mon YYYY" / "DD Month YYYY".
    if tokens.len() >= 3 {
        let joined = format!("{} {} {}", tokens[0], tokens[1], tokens[2]);
        for fmt in ["%d %b %Y", "%d %B %Y"] {
            if let Ok(d) = NaiveDate::parse_from_str(&joined, fmt) {
                return Some((d, 3));
            }
        }
    }
    None
}

/// Parse a money token, tolerating thousands separators, currency words/symbols
/// (KES, Ksh, $), parentheses for negatives, and a trailing DR/CR not attached.
/// Returns `None` for non-money tokens (so description words aren't misread).
fn parse_money(token: &str) -> Option<Decimal> {
    let t = token.trim();
    // Reject obvious non-amounts quickly.
    if t.is_empty() {
        return None;
    }
    // Reject date-like tokens (`31/12/2025`, `2025-12-31`) and slashed refs so a
    // statement period or value date is never misread as a money amount.
    if t.contains('/') || t.matches('-').count() > 1 {
        return None;
    }
    let negative = t.starts_with('(') && t.ends_with(')');
    // Keep digits, dot, minus; drop everything else (commas, currency, parens).
    let cleaned: String = t
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    if cleaned.is_empty() || !cleaned.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    // Require a decimal point OR be a pure integer of reasonable length — avoids
    // treating a reference number like "12345678901234" as an amount when it has
    // no separators. We accept up to 12 integer digits.
    let dot_count = cleaned.matches('.').count();
    if dot_count > 1 {
        return None;
    }
    let int_part = cleaned.split('.').next().unwrap_or("");
    let int_digits = int_part.trim_start_matches('-').len();
    if dot_count == 0 && int_digits > 12 {
        return None;
    }
    let val = Decimal::from_str(&cleaned).ok()?;
    Some(if negative { -val } else { val })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_three_column_kenyan_layout() {
        // Date Description Debit Credit Balance — the common bank PDF table.
        let text = "\
01/06/2026 Opening balance carried forward
02/06/2026 POS PURCHASE NAIVAS 1,200.00 0.00 8,800.00
03/06/2026 SALARY CREDIT EMPLOYER 0.00 50,000.00 58,800.00
04/06/2026 ATM WITHDRAWAL 5,000.00 0.00 53,800.00";
        let rows = parse_statement_text(text);
        assert_eq!(rows.len(), 3, "3 txn rows (opening-balance line has no amounts)");
        assert_eq!(rows[0].debit, Some(dec!(1200.00)));
        assert_eq!(rows[0].credit, None);
        assert_eq!(rows[0].balance, Some(dec!(8800.00)));
        assert_eq!(rows[1].credit, Some(dec!(50000.00)));
        assert_eq!(rows[1].debit, None);
        assert!(rows[0].description.contains("POS PURCHASE"));
    }

    #[test]
    fn parses_amount_balance_with_keyword() {
        let text = "12 Jun 2026 MPESA DEPOSIT received 2,500.00 12,500.00";
        let rows = parse_statement_text(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].credit, Some(dec!(2500.00)));
        assert_eq!(rows[0].balance, Some(dec!(12500.00)));
        assert_eq!(rows[0].value_date, NaiveDate::from_ymd_opt(2026, 6, 12).unwrap());
    }

    #[test]
    fn ignores_non_transaction_lines() {
        let text = "\
ACME BANK LTD — Statement of Account
Account: 1234567890   Period: Jun 2026
Date Description Debit Credit Balance
Page 1 of 3";
        let rows = parse_statement_text(text);
        assert!(rows.is_empty(), "headers/furniture produce no rows");
    }

    #[test]
    fn parenthesised_negative_is_debit() {
        let text = "05/06/2026 BANK CHARGES (350.00) 53,450.00";
        let rows = parse_statement_text(text);
        assert_eq!(rows.len(), 1);
        // Single amount + balance; parens → negative → debit.
        assert_eq!(rows[0].debit, Some(dec!(350.00)));
    }

    #[test]
    fn parse_money_rejects_long_reference_numbers() {
        assert_eq!(parse_money("1234567890123456"), None); // 16-digit ref, no dp
        assert_eq!(parse_money("1,200.00"), Some(dec!(1200.00)));
        assert_eq!(parse_money("KES"), None);
    }

    #[test]
    fn parses_real_mpesa_statement_rows() {
        // Verbatim shape of the M-Pesa merchant XLSX as extracted by xberg.
        let text = "\
M-PESA FULL STATEMENT - Merchant
Receipt No Completion Time Details Transaction Status Paid in Withdrawn Balance Transaction Type Other Party
TLJUJ4PW16 2025-12-19 09:32:46 Biashara Overdraft Repayment Completed 0 55909.78 90.22 OD Payment Transfer 804080-Boost Biashara
TLJ3V1LF5W 2025-12-19 09:19:51 Merchant Payment Online from 254721***933 - JAMES KARANJA MAINA Completed 56000 0 56000 Customer Merchant Payment 7480407-ZAVORA HQ
TL3UJ46MXM 2025-12-03 20:32:28 Biashara Overdraft Repayment Completed 0 300 0 OD Payment Transfer 804080-Boost Biashara";
        let rows = parse_statement_text(text);
        assert_eq!(rows.len(), 3, "3 transaction rows");
        // Row 1: withdrawn 55909.78 → debit; balance 90.22.
        assert_eq!(rows[0].debit, Some(dec!(55909.78)));
        assert_eq!(rows[0].credit, None);
        assert_eq!(rows[0].balance, Some(dec!(90.22)));
        assert_eq!(rows[0].value_date, NaiveDate::from_ymd_opt(2025, 12, 19).unwrap());
        assert!(rows[0].description.to_lowercase().contains("overdraft"));
        // Row 2: paid in 56000 → credit (the 254721***933 ref must NOT be read as an amount).
        assert_eq!(rows[1].credit, Some(dec!(56000)));
        assert_eq!(rows[1].debit, None);
        assert_eq!(rows[1].balance, Some(dec!(56000)));
    }

    #[test]
    fn parses_real_equity_pdf_via_balance_reconciliation() {
        // Verbatim lines as PDFium extracts an Equity Bank (KE) statement: the
        // narrative precedes each txn, and the txn line is
        // `<ref> <date> <amount> <balance>` — the credit/debit *column* is lost,
        // so the side must come from the running-balance delta.
        let text = "\
Transaction Details Payment reference Value Date Credit (Money In) Debit (Money Out) Balance
MPS 254721490933 TGS0WMCLXK JAMES KARANJA MAINA 17
kCEWNjRrzGcO
S85922448 28/07/2025 100.00 118.97
VISA PAYMENT LIMITED CLIENT ACCOUNT
TRNID_43969382
S91586526 31/07/2025 15,575.00 15,693.97
VISA PAYMENT LIMITED CLIENT ACCOUNT
TRNID_43969203
S91586594 31/07/2025 28.00 15,721.97
APP/JAMES KARANJA MAINA/
AD880F6F04B69
54134725 02/08/2025 9,000.00 6,887.97
SMS CHARGE
54134725 02/08/2025 02.26 6,885.71";
        let rows = parse_statement_text(text);
        assert_eq!(rows.len(), 5, "5 transaction rows (date-bearing lines only)");

        // The statement-period / header lines must not become rows.
        // Row 0: first balanced row, no predecessor → kept, low confidence.
        assert_eq!(rows[0].balance, Some(dec!(118.97)));

        // Row 1: 118.97 + 15,575 = 15,693.97 → CREDIT, high confidence.
        assert_eq!(rows[1].credit, Some(dec!(15575.00)));
        assert_eq!(rows[1].debit, None);
        assert!(rows[1].confidence > 0.9);
        // Description pulled from the preceding narrative lines.
        assert!(rows[1].description.to_uppercase().contains("VISA PAYMENT"));

        // Row 3: 15,721.97 − 9,000 = 6,721.97 ≠ 6,887.97 → a row was dropped, but
        // the balance FELL, so the side is still DEBIT (sign trusted, flagged).
        assert_eq!(rows[3].debit, Some(dec!(9000.00)));
        assert!(rows[3].confidence < 0.6, "magnitude mismatch lowers confidence");

        // Row 4: 6,887.97 − 2.26 = 6,885.71 → DEBIT, reconciles exactly.
        assert_eq!(rows[4].debit, Some(dec!(2.26)));
        assert_eq!(rows[4].credit, None);
        assert!(rows[4].confidence > 0.9);
    }

    #[test]
    fn statement_period_line_is_not_a_transaction() {
        // A date range in the header must never be read as a transaction.
        let rows = parse_statement_text("Period 01/01/2025 - 31/12/2025");
        assert!(rows.is_empty());
    }
}
