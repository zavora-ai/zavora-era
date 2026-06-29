use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::bank::*;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};

// ─── Statement Import ────────────────────────────────────────────────────────

/// Import a bank statement file (MT940, OFX, or CSV).
///
/// This function:
/// 1. Detects the file format from content/filename
/// 2. Parses all transaction lines
/// 3. Creates a StatementImport record
/// 4. Inserts each transaction into the categorisation queue (status: uncategorised)
/// 5. Rejects invalid/unparseable files with a descriptive error (no partial records)
///
/// # Idempotency
/// Re-importing the same file content for the same bank account is rejected
/// (file-level `content_hash`). Individual lines that duplicate ones already
/// present are skipped (`dedup_key`). The header + all lines commit in one
/// transaction.
///
/// # Format detection
/// Chosen by filename extension, then content sniffing:
/// - **MT940**: `.mt940` / `.sta` / `.940`, or content starting with `:20:`.
/// - **OFX**: `.ofx` / `.qfx`, or content containing `<OFX>` / `OFXHEADER`.
/// - **CSV**: `.csv`, or comma-separated content with a header row.
///
/// # CSV schema
/// The first row is treated as a header when it contains any of `date`,
/// `description`, `amount`, or `balance` (case-insensitive); otherwise parsing
/// starts at row 1. Columns are **positional**, not name-matched. Dates accept
/// `YYYY-MM-DD`, `DD/MM/YYYY`, or `MM/DD/YYYY`. Supported column layouts:
///
/// | Columns | Layout                                          | Sign convention |
/// |---------|-------------------------------------------------|-----------------|
/// | 3       | `date, description, amount`                     | negative ⇒ debit (money out), positive ⇒ credit (money in) |
/// | 4       | `date, description, amount, balance`            | same as above |
/// | 5+      | `date, description, debit, credit, balance`     | explicit debit & credit columns (blank = none) |
///
/// A separate reference column is **not** parsed; put identifying text in
/// `description`. Every data row must resolve to at least one of debit/credit or
/// the whole import is rejected (no partial imports).
pub async fn import_statement(
    engine: &ErpEngine,
    req: ImportStatementRequest,
) -> ErpResult<ImportStatementResult> {
    // 1. Detect format
    let format = detect_format(&req.filename, &req.content)?;

    // 2. Parse all lines — reject on any parse error (no partial imports)
    let lines = parse_statement(&format, &req.content)?;

    if lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "Statement file contains no transaction lines".to_string(),
        });
    }

    let line_count = lines.len() as u32;
    let import_id = Uuid::new_v4();

    // 3. Create StatementImport record
    let format_str = match format {
        StatementFormat::Mt940 => "mt940",
        StatementFormat::Ofx => "ofx",
        StatementFormat::Csv => "csv",
        StatementFormat::Api => "api",
    };

    // File-level idempotency: a stable hash of the raw content. Re-importing the
    // exact same file for the same bank account is rejected so the categorisation
    // queue (and any downstream GL postings) cannot be silently doubled.
    let content_hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        req.content.hash(&mut h);
        format!("{:016x}", h.finish())
    };

    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM statement_imports WHERE entity_id = $1 AND bank_account_id = $2 AND content_hash = $3",
    )
    .bind(req.entity_id)
    .bind(req.bank_account_id)
    .bind(&content_hash)
    .fetch_optional(engine.pool())
    .await?;
    if existing.is_some() {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "This statement file has already been imported for this bank account ({} lines). Re-importing is blocked to prevent duplicate transactions.",
                line_count
            ),
        });
    }

    // All-or-nothing: the import header and every line commit together.
    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        r#"INSERT INTO statement_imports (id, entity_id, bank_account_id, format, filename, imported_at, line_count, matched_count, unmatched_count, content_hash)
           VALUES ($1, $2, $3, $4, $5, NOW(), $6, 0, 0, $7)"#,
    )
    .bind(import_id)
    .bind(req.entity_id)
    .bind(req.bank_account_id)
    .bind(format_str)
    .bind(&req.filename)
    .bind(line_count as i32)
    .bind(&content_hash)
    .execute(&mut *tx)
    .await?;

    // 4. Insert each transaction line into categorisation queue, skipping any
    // line that duplicates one already present for this bank account (line-level
    // dedup via a deterministic key + ON CONFLICT DO NOTHING).
    let mut inserted = 0u32;
    for (idx, line) in lines.iter().enumerate() {
        let txn_id = Uuid::new_v4();
        let dedup_key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            line.value_date.hash(&mut h);
            line.reference.hash(&mut h);
            line.description.hash(&mut h);
            line.debit.map(|d| d.to_string()).hash(&mut h);
            line.credit.map(|c| c.to_string()).hash(&mut h);
            // Include the within-file position so legitimately identical lines in
            // one statement (e.g. two equal fees same day) are preserved, while a
            // re-imported file (same positions) still collides.
            idx.hash(&mut h);
            format!("{:016x}", h.finish())
        };
        let res = sqlx::query(
            r#"INSERT INTO imported_transactions 
               (id, entity_id, bank_account, value_date, description, reference, debit, credit, running_bal, category_status, import_batch_id, created_at, dedup_key)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'uncategorised', $10, NOW(), $11)
               ON CONFLICT (entity_id, bank_account, dedup_key) WHERE dedup_key IS NOT NULL DO NOTHING"#,
        )
        .bind(txn_id)
        .bind(req.entity_id)
        .bind(req.bank_account_id)
        .bind(line.value_date)
        .bind(&line.description)
        .bind(&line.reference)
        .bind(line.debit)
        .bind(line.credit)
        .bind(line.balance.unwrap_or(Decimal::ZERO))
        .bind(import_id)
        .bind(&dedup_key)
        .execute(&mut *tx)
        .await?;
        inserted += res.rows_affected() as u32;
    }

    tx.commit().await?;

    Ok(ImportStatementResult {
        import_id,
        format,
        line_count: inserted,
        matched_count: 0,
        unmatched_count: 0,
    })
}

/// Detect statement format from filename extension and content heuristics.
fn detect_format(filename: &str, content: &str) -> ErpResult<StatementFormat> {
    let lower = filename.to_lowercase();

    // Check extension first
    if lower.ends_with(".mt940") || lower.ends_with(".sta") || lower.ends_with(".940") {
        return Ok(StatementFormat::Mt940);
    }
    if lower.ends_with(".ofx") || lower.ends_with(".qfx") {
        return Ok(StatementFormat::Ofx);
    }
    if lower.ends_with(".csv") {
        return Ok(StatementFormat::Csv);
    }

    // Fallback: inspect content
    let trimmed = content.trim();
    if trimmed.starts_with(":20:") || trimmed.contains("\n:20:") {
        return Ok(StatementFormat::Mt940);
    }
    if trimmed.contains("<OFX>") || trimmed.contains("<ofx>") || trimmed.contains("OFXHEADER") {
        return Ok(StatementFormat::Ofx);
    }
    // If it has commas and newlines with a header-like first line, assume CSV
    if trimmed.contains(',') && trimmed.contains('\n') {
        return Ok(StatementFormat::Csv);
    }

    Err(ErpError::ValidationFailed {
        message: format!(
            "Unable to determine statement format for file '{}'. Supported formats: MT940, OFX, CSV.",
            filename
        ),
    })
}

/// Parse statement content based on detected format.
fn parse_statement(format: &StatementFormat, content: &str) -> ErpResult<Vec<ParsedStatementLine>> {
    match format {
        StatementFormat::Mt940 => parse_mt940(content),
        StatementFormat::Ofx => parse_ofx(content),
        StatementFormat::Csv => parse_csv(content),
        StatementFormat::Api => Err(ErpError::ValidationFailed {
            message: "API format is for automated feeds, not file imports".to_string(),
        }),
    }
}

/// Parse MT940 (SWIFT) bank statement format.
///
/// MT940 uses tagged fields:
/// - :20: Transaction reference
/// - :25: Account identification
/// - :60F: Opening balance
/// - :61: Statement line (transaction)
/// - :86: Transaction info/description
/// - :62F: Closing balance
fn parse_mt940(content: &str) -> ErpResult<Vec<ParsedStatementLine>> {
    let mut lines: Vec<ParsedStatementLine> = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut current_amount: Option<(Option<Decimal>, Option<Decimal>)> = None;
    let mut current_ref = String::new();
    let mut pending_description = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();

        if line.starts_with(":61:") {
            // Commit previous transaction if pending (no :86: line followed)
            if let Some((debit, credit)) = current_amount.take() {
                lines.push(ParsedStatementLine {
                    value_date: current_date.unwrap_or(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
                    description: String::new(),
                    reference: current_ref.clone(),
                    debit,
                    credit,
                    balance: None,
                });
            }

            // Parse :61: line — format: YYMMDD[MMDD]D/C[amount][reference]
            let field = &line[4..];
            if field.len() < 16 {
                return Err(ErpError::ValidationFailed {
                    message: format!("Invalid MT940 :61: line too short: '{}'", line),
                });
            }

            // Parse date (YYMMDD)
            let date_str = &field[..6];
            current_date = Some(parse_mt940_date(date_str).map_err(|e| {
                ErpError::ValidationFailed {
                    message: format!("Invalid date in MT940 :61: line: '{}' — {}", date_str, e),
                }
            })?);

            // Find debit/credit indicator and amount
            // After the date(s), there's a D or C (or RD/RC for reversal), then the amount
            let after_date = if field.len() > 10 && field.chars().nth(6).map_or(false, |c| c.is_ascii_digit()) {
                // Has booking date (MMDD) after value date
                &field[10..]
            } else {
                &field[6..]
            };

            let (is_debit, amount_str) = parse_mt940_amount_field(after_date).map_err(|e| {
                ErpError::ValidationFailed {
                    message: format!("Invalid MT940 amount in :61: line: '{}' — {}", line, e),
                }
            })?;

            let amount: Decimal = amount_str.replace(',', ".").parse().map_err(|_| {
                ErpError::ValidationFailed {
                    message: format!("Invalid MT940 amount value: '{}'", amount_str),
                }
            })?;

            if is_debit {
                current_amount = Some((Some(amount), None));
            } else {
                current_amount = Some((None, Some(amount)));
            }

            // Extract reference (everything after the amount field identifier)
            current_ref = extract_mt940_reference(after_date);
            pending_description = true;
        } else if line.starts_with(":86:") && pending_description {
            // Description for previous :61: line
            let description = line[4..].to_string();
            if let Some((debit, credit)) = current_amount.take() {
                lines.push(ParsedStatementLine {
                    value_date: current_date.unwrap_or(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
                    description,
                    reference: current_ref.clone(),
                    debit,
                    credit,
                    balance: None,
                });
                pending_description = false;
            }
        }
    }

    // Commit last transaction if still pending
    if let Some((debit, credit)) = current_amount.take() {
        lines.push(ParsedStatementLine {
            value_date: current_date.unwrap_or(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
            description: String::new(),
            reference: current_ref,
            debit,
            credit,
            balance: None,
        });
    }

    if lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "MT940 file contains no valid transaction lines (:61: fields)".to_string(),
        });
    }

    Ok(lines)
}

/// Parse an MT940 date in YYMMDD format.
fn parse_mt940_date(s: &str) -> Result<NaiveDate, String> {
    if s.len() != 6 {
        return Err(format!("Expected 6 chars, got {}", s.len()));
    }
    let year: i32 = s[..2].parse().map_err(|_| "invalid year")?;
    let month: u32 = s[2..4].parse().map_err(|_| "invalid month")?;
    let day: u32 = s[4..6].parse().map_err(|_| "invalid day")?;

    // Two-digit year: 00-49 → 2000s, 50-99 → 1900s
    let full_year = if year < 50 { 2000 + year } else { 1900 + year };

    NaiveDate::from_ymd_opt(full_year, month, day)
        .ok_or_else(|| format!("invalid date: {}-{:02}-{:02}", full_year, month, day))
}

/// Parse the amount field from an MT940 :61: line (after date section).
/// Returns (is_debit, amount_string).
fn parse_mt940_amount_field(s: &str) -> Result<(bool, String), String> {
    if s.is_empty() {
        return Err("empty amount field".to_string());
    }

    let (is_debit, rest) = if s.starts_with("RD") || s.starts_with("rd") {
        (true, &s[2..])
    } else if s.starts_with("RC") || s.starts_with("rc") {
        (false, &s[2..])
    } else if s.starts_with('D') || s.starts_with('d') {
        (true, &s[1..])
    } else if s.starts_with('C') || s.starts_with('c') {
        (false, &s[1..])
    } else {
        return Err(format!("expected D/C/RD/RC indicator, got: '{}'", &s[..1]));
    };

    // Amount ends at first non-digit/non-comma/non-dot character after the first digit
    let amount_end = rest
        .find(|c: char| !c.is_ascii_digit() && c != ',' && c != '.')
        .unwrap_or(rest.len());

    let amount_str = &rest[..amount_end];
    if amount_str.is_empty() {
        return Err("no amount value found".to_string());
    }

    Ok((is_debit, amount_str.to_string()))
}

/// Extract reference from the MT940 :61: field (after amount).
fn extract_mt940_reference(s: &str) -> String {
    // Skip D/C indicator and amount to get to reference
    let after_indicator = if s.starts_with("RD") || s.starts_with("RC") || s.starts_with("rd") || s.starts_with("rc") {
        &s[2..]
    } else if s.starts_with('D') || s.starts_with('d') || s.starts_with('C') || s.starts_with('c') {
        &s[1..]
    } else {
        s
    };

    // Skip digits, commas, dots (the amount)
    let ref_start = after_indicator
        .find(|c: char| !c.is_ascii_digit() && c != ',' && c != '.')
        .unwrap_or(after_indicator.len());

    after_indicator[ref_start..].trim().to_string()
}

/// Parse OFX (Open Financial Exchange) format.
///
/// OFX is XML-based with <STMTTRN> elements containing:
/// - <DTPOSTED> date
/// - <TRNAMT> amount (negative = debit, positive = credit)
/// - <NAME> or <MEMO> description
/// - <FITID> or <CHECKNUM> reference
fn parse_ofx(content: &str) -> ErpResult<Vec<ParsedStatementLine>> {
    let mut lines: Vec<ParsedStatementLine> = Vec::new();

    // Simple tag-based parsing (OFX doesn't always use closing tags)
    let upper = content.to_uppercase();
    if !upper.contains("<STMTTRN>") && !upper.contains("<STMTTRNP>") {
        return Err(ErpError::ValidationFailed {
            message: "OFX file contains no transaction records (<STMTTRN> elements)".to_string(),
        });
    }

    // Find all STMTTRN blocks (case-insensitive)
    let mut search_from = 0;
    loop {
        let upper_slice = &upper[search_from..];
        let start = match upper_slice.find("<STMTTRN>") {
            Some(pos) => search_from + pos,
            None => break,
        };

        let end_tag_pos = upper[start..].find("</STMTTRN>");
        let block_end = match end_tag_pos {
            Some(pos) => start + pos,
            None => {
                // OFX sometimes uses next <STMTTRN> as delimiter
                match upper[start + 9..].find("<STMTTRN>") {
                    Some(pos) => start + 9 + pos,
                    None => content.len(),
                }
            }
        };

        let block = &content[start..block_end];

        let date = extract_ofx_tag(block, "DTPOSTED")
            .and_then(|d| parse_ofx_date(&d));
        let amount_str = extract_ofx_tag(block, "TRNAMT");
        let name = extract_ofx_tag(block, "NAME").unwrap_or_default();
        let memo = extract_ofx_tag(block, "MEMO").unwrap_or_default();
        let fitid = extract_ofx_tag(block, "FITID").unwrap_or_default();
        let checknum = extract_ofx_tag(block, "CHECKNUM").unwrap_or_default();

        let value_date = match date {
            Some(d) => d,
            None => {
                return Err(ErpError::ValidationFailed {
                    message: "OFX transaction missing or invalid DTPOSTED date".to_string(),
                });
            }
        };

        let amount: Decimal = match amount_str {
            Some(ref s) => s.trim().parse().map_err(|_| ErpError::ValidationFailed {
                message: format!("Invalid OFX amount value: '{}'", s),
            })?,
            None => {
                return Err(ErpError::ValidationFailed {
                    message: "OFX transaction missing TRNAMT field".to_string(),
                });
            }
        };

        let description = if !name.is_empty() { name } else { memo };
        let reference = if !fitid.is_empty() { fitid } else { checknum };

        let (debit, credit) = if amount < Decimal::ZERO {
            (Some(-amount), None) // negative = money out = debit
        } else {
            (None, Some(amount)) // positive = money in = credit
        };

        lines.push(ParsedStatementLine {
            value_date,
            description,
            reference,
            debit,
            credit,
            balance: None,
        });

        search_from = block_end;
    }

    if lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "OFX file contains no parseable transaction records".to_string(),
        });
    }

    Ok(lines)
}

/// Extract a tag value from an OFX block.
/// OFX tags don't always have closing tags, value is until next < or newline.
fn extract_ofx_tag(block: &str, tag: &str) -> Option<String> {
    let upper_block = block.to_uppercase();
    let search = format!("<{}>", tag.to_uppercase());
    let pos = upper_block.find(&search)?;
    let value_start = pos + search.len();
    let remaining = &block[value_start..];

    // Value ends at next '<' or newline
    let value_end = remaining
        .find(|c: char| c == '<' || c == '\n' || c == '\r')
        .unwrap_or(remaining.len());

    let value = remaining[..value_end].trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

/// Parse OFX date format (YYYYMMDD or YYYYMMDDHHMMSS).
fn parse_ofx_date(s: &str) -> Option<NaiveDate> {
    if s.len() < 8 {
        return None;
    }
    let date_part = &s[..8];
    let year: i32 = date_part[..4].parse().ok()?;
    let month: u32 = date_part[4..6].parse().ok()?;
    let day: u32 = date_part[6..8].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Parse CSV bank statement format.
///
/// Expected columns: date, description, amount (or debit/credit), balance
/// Supports common variations:
/// - 3 columns: date, description, amount (negative=debit, positive=credit)
/// - 4 columns: date, description, amount, balance
/// - 5 columns: date, description, debit, credit, balance
fn parse_csv(content: &str) -> ErpResult<Vec<ParsedStatementLine>> {
    let mut lines: Vec<ParsedStatementLine> = Vec::new();
    let all_lines: Vec<&str> = content.lines().collect();

    if all_lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "CSV file is empty".to_string(),
        });
    }

    // Skip header row (first line)
    let header = all_lines[0].to_lowercase();
    let data_start = if header.contains("date") || header.contains("description") || header.contains("amount") || header.contains("balance") {
        1
    } else {
        0
    };

    if all_lines.len() <= data_start {
        return Err(ErpError::ValidationFailed {
            message: "CSV file contains no data rows".to_string(),
        });
    }

    // Detect column count from first data row
    let first_data = all_lines[data_start];
    let col_count = csv_split(first_data).len();

    for (row_idx, &row) in all_lines[data_start..].iter().enumerate() {
        let trimmed = row.trim();
        if trimmed.is_empty() {
            continue;
        }

        let cols = csv_split(trimmed);
        if cols.len() < 3 {
            return Err(ErpError::ValidationFailed {
                message: format!(
                    "CSV row {} has {} columns, expected at least 3 (date, description, amount). Row: '{}'",
                    row_idx + data_start + 1,
                    cols.len(),
                    trimmed
                ),
            });
        }

        let value_date = parse_csv_date(cols[0].trim()).map_err(|e| ErpError::ValidationFailed {
            message: format!("Invalid date '{}' in CSV row {}: {}", cols[0].trim(), row_idx + data_start + 1, e),
        })?;

        let description = cols[1].trim().trim_matches('"').to_string();

        let (debit, credit, balance) = if col_count >= 5 {
            // 5+ columns: date, description, debit, credit, balance
            let d = parse_csv_amount(cols[2].trim());
            let c = parse_csv_amount(cols[3].trim());
            let b = parse_csv_amount(cols[4].trim());
            (d, c, b)
        } else if col_count == 4 {
            // 4 columns: date, description, amount, balance
            let amount = parse_csv_amount(cols[2].trim());
            let b = parse_csv_amount(cols[3].trim());
            match amount {
                Some(a) if a < Decimal::ZERO => (Some(-a), None, b),
                Some(a) => (None, Some(a), b),
                None => (None, None, b),
            }
        } else {
            // 3 columns: date, description, amount
            let amount = parse_csv_amount(cols[2].trim());
            match amount {
                Some(a) if a < Decimal::ZERO => (Some(-a), None, None),
                Some(a) => (None, Some(a), None),
                None => {
                    return Err(ErpError::ValidationFailed {
                        message: format!(
                            "Invalid amount '{}' in CSV row {}",
                            cols[2].trim(),
                            row_idx + data_start + 1
                        ),
                    });
                }
            }
        };

        // At least one of debit/credit must be present
        if debit.is_none() && credit.is_none() {
            return Err(ErpError::ValidationFailed {
                message: format!(
                    "CSV row {} has no valid debit or credit amount",
                    row_idx + data_start + 1
                ),
            });
        }

        lines.push(ParsedStatementLine {
            value_date,
            description: description.clone(),
            reference: String::new(), // CSV typically doesn't have a separate reference
            debit,
            credit,
            balance,
        });
    }

    if lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "CSV file contains no valid transaction rows".to_string(),
        });
    }

    Ok(lines)
}

/// Split a CSV line respecting quoted fields.
fn csv_split(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;

    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    fields.push(&line[start..]);
    fields
}

/// Parse a CSV date — supports common formats.
fn parse_csv_date(s: &str) -> Result<NaiveDate, String> {
    let cleaned = s.trim_matches('"').trim();

    // Try YYYY-MM-DD
    if let Ok(d) = NaiveDate::parse_from_str(cleaned, "%Y-%m-%d") {
        return Ok(d);
    }
    // Try DD/MM/YYYY
    if let Ok(d) = NaiveDate::parse_from_str(cleaned, "%d/%m/%Y") {
        return Ok(d);
    }
    // Try MM/DD/YYYY
    if let Ok(d) = NaiveDate::parse_from_str(cleaned, "%m/%d/%Y") {
        return Ok(d);
    }
    // Try DD-MM-YYYY
    if let Ok(d) = NaiveDate::parse_from_str(cleaned, "%d-%m-%Y") {
        return Ok(d);
    }
    // Try YYYYMMDD
    if cleaned.len() == 8 && cleaned.chars().all(|c| c.is_ascii_digit()) {
        let y: i32 = cleaned[..4].parse().map_err(|_| "invalid year")?;
        let m: u32 = cleaned[4..6].parse().map_err(|_| "invalid month")?;
        let d: u32 = cleaned[6..8].parse().map_err(|_| "invalid day")?;
        return NaiveDate::from_ymd_opt(y, m, d)
            .ok_or_else(|| format!("invalid date components: {}-{}-{}", y, m, d));
    }

    Err(format!("unrecognised date format: '{}'", cleaned))
}

/// Parse a CSV amount value, returning None for empty strings.
fn parse_csv_amount(s: &str) -> Option<Decimal> {
    let cleaned = s.trim_matches('"').trim().replace(' ', "");
    if cleaned.is_empty() || cleaned == "-" {
        return None;
    }
    cleaned.parse::<Decimal>().ok()
}

// ─── Three-Pass Reconciliation ───────────────────────────────────────────────

/// Run the three-pass bank reconciliation matching algorithm.
///
/// Pass 1: Exact match — stmt.amount = je.amount AND stmt.date = je.date AND stmt.reference = je.reference
/// Pass 2: Near match — stmt.amount = je.amount AND |stmt.date - je.date| <= 3 days AND fuzzy(stmt.ref, je.ref) > 0.8
/// Pass 3: AI suggestion — remaining unmatched lines get account suggestions from historical categorisations
pub async fn match_bank_lines(engine: &ErpEngine, entity_id: Uuid, statement_id: Uuid) -> ErpResult<MatchReport> {
    // ─── Pass 1: Exact Match ─────────────────────────────────────────────────
    let exact_matches = sqlx::query_as::<_, ExactMatchRow>(
        r#"SELECT DISTINCT ON (it.id)
               it.id as stmt_line_id, je.id as journal_entry_id,
               COALESCE(it.debit, it.credit) as amount, it.value_date as date
           FROM imported_transactions it
           JOIN journal_entries je ON je.entity_id = it.entity_id AND je.status = 'posted'
           JOIN journal_lines jl ON jl.entry_id = je.id
           WHERE it.import_batch_id = $1 AND it.entity_id = $2 AND it.category_status = 'uncategorised'
           AND je.date = it.value_date
           AND (jl.functional_debit = it.credit OR jl.functional_credit = it.debit)
           AND je.reference = it.reference
           AND je.reference != ''"#,
    )
    .bind(statement_id)
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let exact: Vec<MatchPair> = exact_matches
        .iter()
        .map(|r| MatchPair {
            statement_line_id: r.stmt_line_id,
            journal_entry_id: r.journal_entry_id,
            amount: r.amount,
            date: r.date,
        })
        .collect();

    // Collect IDs already matched in Pass 1 so they are excluded from Pass 2
    let matched_stmt_ids: Vec<Uuid> = exact.iter().map(|m| m.statement_line_id).collect();
    let matched_je_ids: Vec<Uuid> = exact.iter().map(|m| m.journal_entry_id).collect();

    // ─── Pass 2: Near Match ──────────────────────────────────────────────────
    // Find candidates where amount matches and date is within 3 days
    let near_candidates = sqlx::query_as::<_, NearMatchCandidateRow>(
        r#"SELECT DISTINCT ON (it.id)
               it.id as stmt_line_id, je.id as journal_entry_id,
               COALESCE(it.debit, it.credit) as amount,
               it.value_date as stmt_date, je.date as je_date,
               it.reference as stmt_reference, je.reference as je_reference
           FROM imported_transactions it
           JOIN journal_entries je ON je.entity_id = it.entity_id AND je.status = 'posted'
           JOIN journal_lines jl ON jl.entry_id = je.id
           WHERE it.import_batch_id = $1 AND it.entity_id = $4 AND it.category_status = 'uncategorised'
           AND (jl.functional_debit = it.credit OR jl.functional_credit = it.debit)
           AND ABS(je.date - it.value_date) <= 3
           AND it.id != ALL($2)
           AND je.id != ALL($3)"#,
    )
    .bind(statement_id)
    .bind(&matched_stmt_ids)
    .bind(&matched_je_ids)
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let mut near_matches: Vec<NearMatch> = Vec::new();
    let mut pass2_stmt_ids: Vec<Uuid> = Vec::new();

    for candidate in &near_candidates {
        // Skip if this statement line was already matched in a prior near-match iteration
        if pass2_stmt_ids.contains(&candidate.stmt_line_id) {
            continue;
        }

        let similarity = fuzzy_reference_similarity(
            &candidate.stmt_reference,
            &candidate.je_reference,
        );

        if similarity > 0.8 {
            let date_diff = (candidate.je_date - candidate.stmt_date).num_days().unsigned_abs() as i32;
            near_matches.push(NearMatch {
                statement_line_id: candidate.stmt_line_id,
                journal_entry_id: candidate.journal_entry_id,
                amount: candidate.amount,
                date_diff_days: date_diff,
                reference_similarity: similarity,
            });
            pass2_stmt_ids.push(candidate.stmt_line_id);
        }
    }

    // Collect all matched IDs from Pass 1 + Pass 2 for Pass 3 exclusion
    let all_matched_stmt_ids: Vec<Uuid> = matched_stmt_ids
        .iter()
        .chain(pass2_stmt_ids.iter())
        .copied()
        .collect();

    // ─── Pass 3: AI Suggestion ───────────────────────────────────────────────
    // For remaining unmatched lines, suggest based on historical categorisations
    let unmatched_lines = sqlx::query_as::<_, UnmatchedLineRow>(
        r#"SELECT id, entity_id, description, reference, debit, credit, value_date
           FROM imported_transactions
           WHERE import_batch_id = $1 AND entity_id = $3 AND category_status = 'uncategorised'
           AND id != ALL($2)"#,
    )
    .bind(statement_id)
    .bind(&all_matched_stmt_ids)
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let mut ai_suggestions: Vec<AiSuggestion> = Vec::new();
    let mut unmatched_ids: Vec<Uuid> = Vec::new();

    for line in &unmatched_lines {
        // Look for historical categorisations with similar description
        let suggestion = suggest_from_history(engine, line).await;
        match suggestion {
            Some(s) => ai_suggestions.push(s),
            None => unmatched_ids.push(line.id),
        }
    }

    Ok(MatchReport {
        statement_id,
        exact_matches: exact,
        near_matches,
        ai_suggestions,
        unmatched: unmatched_ids,
    })
}

/// Confirm a reconciliation match — links statement line to journal entry
/// and marks both as reconciled.
pub async fn confirm_match(engine: &ErpEngine, entity_id: Uuid, req: ConfirmMatchRequest) -> ErpResult<()> {
    let mut tx = engine.pool().begin().await?;

    // Link statement line to journal entry and mark as reconciled/posted
    sqlx::query(
        r#"UPDATE imported_transactions 
           SET journal_entry_id = $1, category_status = 'posted'
           WHERE id = $2 AND category_status IN ('uncategorised', 'suggested')"#,
    )
    .bind(req.journal_entry_id)
    .bind(req.statement_line_id)
    .execute(&mut *tx)
    .await?;

    // Mark the journal entry as reconciled (set reconciled flag)
    sqlx::query(
        r#"UPDATE journal_entries 
           SET reconciled = true, reconciled_at = NOW()
           WHERE id = $1"#,
    )
    .bind(req.journal_entry_id)
    .execute(&mut *tx)
    .await?;

    // Update the import batch matched/unmatched counts
    sqlx::query(
        r#"UPDATE statement_imports si
           SET matched_count = matched_count + 1
           WHERE id = (
               SELECT import_batch_id FROM imported_transactions WHERE id = $1
           )"#,
    )
    .bind(req.statement_line_id)
    .execute(&mut *tx)
    .await?;

    // Emit audit event
    let audit_event = serde_json::json!({
        "event_type": "reconciliation_match_confirmed",
        "object_type": "imported_transaction",
        "object_id": req.statement_line_id,
        "journal_entry_id": req.journal_entry_id,
        "actor": req.confirmed_by,
        "timestamp": chrono::Utc::now(),
    });

    let stream_key = format!("erp:audit:{}", entity_id);
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;

    tx.commit().await?;

    Ok(())
}

/// Post an unmatched bank line as a new journal entry and link it to the statement line.
///
/// Creates a JE with:
/// - If the transaction is a credit (money in): DR Bank account / CR assigned account
/// - If the transaction is a debit (money out): DR assigned account / CR Bank account
pub async fn post_unmatched(engine: &ErpEngine, entity_id: Uuid, req: PostUnmatchedRequest) -> ErpResult<Uuid> {
    // Fetch the unmatched transaction
    let txn = sqlx::query_as::<_, crate::transactions::ImportedTransactionRow>(
        "SELECT * FROM imported_transactions WHERE id = $1",
    )
    .bind(req.statement_line_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "ImportedTransaction".to_string(),
        id: req.statement_line_id,
    })?;

    // Determine the bank GL account for this bank account, falling back to the
    // tenant's configured default bank account (not a hardcoded code).
    let bank_gl_account = match sqlx::query_scalar::<_, String>(
        "SELECT gl_account FROM bank_accounts WHERE id = $1 AND entity_id = $2",
    )
    .bind(txn.bank_account)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    {
        Some(a) => a,
        None => engine.posting_for(entity_id).await?.default_bank.clone(),
    };

    let amount = txn.debit.or(txn.credit).unwrap_or(Decimal::ZERO);
    if amount == Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: "Transaction has no debit or credit amount".to_string(),
        });
    }

    let base_currency = engine.config_for(entity_id).await?.base_currency.clone();

    // Build journal entry lines based on whether it's a debit or credit transaction
    let lines = if txn.debit.is_some() {
        // Money out: DR assigned expense/account / CR Bank
        vec![
            CreateJournalLineRequest {
                account_code: req.account_code.clone(),
                debit: Some(amount),
                credit: None,
                currency: base_currency.clone(),
                fx_rate: None,
                description: Some(req.description.clone()),
                dimensions: None,
            },
            CreateJournalLineRequest {
                account_code: bank_gl_account,
                debit: None,
                credit: Some(amount),
                currency: base_currency,
                fx_rate: None,
                description: Some(req.description.clone()),
                dimensions: None,
            },
        ]
    } else {
        // Money in: DR Bank / CR assigned revenue/account
        vec![
            CreateJournalLineRequest {
                account_code: bank_gl_account,
                debit: Some(amount),
                credit: None,
                currency: base_currency.clone(),
                fx_rate: None,
                description: Some(req.description.clone()),
                dimensions: None,
            },
            CreateJournalLineRequest {
                account_code: req.account_code.clone(),
                debit: None,
                credit: Some(amount),
                currency: base_currency,
                fx_rate: None,
                description: Some(req.description.clone()),
                dimensions: None,
            },
        ]
    };

    // Resolve period for the transaction date
    let period = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM fiscal_periods WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
    )
    .bind(entity_id)
    .bind(txn.value_date)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::ValidationFailed {
        message: format!("No fiscal period found for date {}", txn.value_date),
    })?;

    // Create the journal entry via the journal service
    let je_request = CreateJournalEntryRequest {
        date: txn.value_date,
        source: JournalSource::Payment,
        source_id: None,
        reference: txn.reference.clone(),
        description: req.description.clone(),
        lines,
        post_immediately: true,
    };

    let je = crate::services::journal::create_and_post(engine, entity_id, je_request, period, req.posted_by.clone()).await?;

    // Link the transaction to the new journal entry and mark as posted
    sqlx::query(
        r#"UPDATE imported_transactions 
           SET journal_entry_id = $1, assigned_account = $2, category_status = 'posted' 
           WHERE id = $3"#,
    )
    .bind(je.id)
    .bind(&req.account_code)
    .bind(req.statement_line_id)
    .execute(engine.pool())
    .await?;

    // Update import batch counts
    sqlx::query(
        r#"UPDATE statement_imports 
           SET matched_count = matched_count + 1 
           WHERE id = $1"#,
    )
    .bind(txn.import_batch_id)
    .execute(engine.pool())
    .await?;

    Ok(je.id)
}

/// Verify that the statement balance equals the GL balance for a bank account
/// after reconciliation. Reports the difference if they don't match.
///
/// Returns a ReconciliationSummary with the reconciliation status.
pub async fn verify_reconciliation_balance(
    engine: &ErpEngine,
    entity_id: Uuid,
    statement_id: Uuid,
) -> ErpResult<ReconciliationSummary> {
    // Get statement import details
    let import = sqlx::query_as::<_, StatementImportRow>(
        "SELECT * FROM statement_imports WHERE id = $1 AND entity_id = $2",
    )
    .bind(statement_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "StatementImport".to_string(),
        id: statement_id,
    })?;

    // Compute the statement balance: sum of credits minus sum of debits from imported lines
    let statement_balance = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(credit, 0)) - SUM(COALESCE(debit, 0)), 0)
           FROM imported_transactions
           WHERE import_batch_id = $1 AND category_status = 'posted'"#,
    )
    .bind(statement_id)
    .fetch_one(engine.pool())
    .await?;

    // Compute the GL balance for this bank account:
    // Sum of functional debits minus functional credits on the bank GL account
    let gl_balance = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0)) - SUM(COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           WHERE je.entity_id = $1
           AND je.status = 'posted'
           AND jl.account_code = (SELECT gl_account FROM bank_accounts WHERE id = $2 AND entity_id = $1)
           AND je.reconciled = true"#,
    )
    .bind(entity_id)
    .bind(import.bank_account_id)
    .fetch_one(engine.pool())
    .await?;

    let difference = statement_balance - gl_balance;
    let is_reconciled = difference == Decimal::ZERO;

    // Count matched vs unmatched
    let matched_lines = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM imported_transactions WHERE import_batch_id = $1 AND category_status = 'posted'",
    )
    .bind(statement_id)
    .fetch_one(engine.pool())
    .await? as u32;

    let unmatched_lines = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM imported_transactions WHERE import_batch_id = $1 AND category_status != 'posted'",
    )
    .bind(statement_id)
    .fetch_one(engine.pool())
    .await? as u32;

    // If fully reconciled, mark the import as complete
    if is_reconciled && unmatched_lines == 0 {
        sqlx::query(
            "UPDATE statement_imports SET matched_count = $1, unmatched_count = 0 WHERE id = $2",
        )
        .bind(matched_lines as i32)
        .bind(statement_id)
        .execute(engine.pool())
        .await?;
    } else if !is_reconciled {
        // Report the difference — do not mark as fully reconciled
        sqlx::query(
            "UPDATE statement_imports SET matched_count = $1, unmatched_count = $2 WHERE id = $3",
        )
        .bind(matched_lines as i32)
        .bind(unmatched_lines as i32)
        .bind(statement_id)
        .execute(engine.pool())
        .await?;
    }

    Ok(ReconciliationSummary {
        bank_account_id: import.bank_account_id,
        statement_id,
        statement_balance,
        gl_balance,
        difference,
        matched_lines,
        unmatched_lines,
        is_reconciled,
    })
}

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Compute fuzzy reference similarity between two strings using a simplified
/// trigram-based approach. Returns a value between 0.0 and 1.0.
fn fuzzy_reference_similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match fast path
    if a_lower == b_lower {
        return 1.0;
    }

    // Generate trigrams for both strings
    let trigrams_a = generate_trigrams(&a_lower);
    let trigrams_b = generate_trigrams(&b_lower);

    if trigrams_a.is_empty() && trigrams_b.is_empty() {
        // Strings too short for trigrams, fall back to character comparison
        let common = a_lower
            .chars()
            .zip(b_lower.chars())
            .filter(|(ca, cb)| ca == cb)
            .count();
        let max_len = a_lower.len().max(b_lower.len());
        return common as f32 / max_len as f32;
    }

    // Count matching trigrams
    let matching = trigrams_a.iter().filter(|t| trigrams_b.contains(t)).count();
    let total = trigrams_a.len().max(trigrams_b.len());

    if total == 0 {
        return 0.0;
    }

    matching as f32 / total as f32
}

/// Generate trigrams (3-character substrings) from a string.
fn generate_trigrams(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        return Vec::new();
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Suggest an account code based on historical categorisations of similar transactions.
/// Uses description similarity to find past categorisations and suggests the most common account.
async fn suggest_from_history(engine: &ErpEngine, line: &UnmatchedLineRow) -> Option<AiSuggestion> {
    // Query historical categorisations for this entity — posted transactions with assigned accounts
    let history = sqlx::query_as::<_, HistoricalCategorisationRow>(
        r#"SELECT description, assigned_account 
           FROM imported_transactions 
           WHERE entity_id = $1 
           AND category_status = 'posted' 
           AND assigned_account IS NOT NULL
           ORDER BY created_at DESC
           LIMIT 200"#,
    )
    .bind(line.entity_id)
    .fetch_all(engine.pool())
    .await
    .ok()?;

    if history.is_empty() {
        return None;
    }

    // Find the best matching historical description
    let mut best_account: Option<String> = None;
    let mut best_similarity: f32 = 0.0;
    let mut account_counts: std::collections::HashMap<String, (u32, f32)> = std::collections::HashMap::new();

    for entry in &history {
        let similarity = fuzzy_reference_similarity(&line.description, &entry.description);
        if similarity > 0.5 {
            let counter = account_counts.entry(entry.assigned_account.clone()).or_insert((0, 0.0));
            counter.0 += 1;
            if similarity > counter.1 {
                counter.1 = similarity;
            }
            if similarity > best_similarity {
                best_similarity = similarity;
                best_account = Some(entry.assigned_account.clone());
            }
        }
    }

    // Only suggest if we found a reasonable match
    let suggested = best_account?;
    if best_similarity < 0.5 {
        return None;
    }

    // Confidence ranges from 0.5 to 0.9 based on similarity and frequency
    let count = account_counts.get(&suggested).map(|(c, _)| *c).unwrap_or(1);
    let frequency_boost = (count as f32 / 10.0).min(0.2); // max 0.2 boost for frequency
    let confidence = (best_similarity * 0.7 + frequency_boost + 0.1).min(0.9).max(0.5);

    Some(AiSuggestion {
        statement_line_id: line.id,
        suggested_account: suggested,
        confidence,
        reason: format!(
            "Similar to {} historical transaction(s) with description matching at {:.0}%",
            count,
            best_similarity * 100.0
        ),
    })
}

// ─── Internal Row Types ──────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct ExactMatchRow {
    stmt_line_id: Uuid,
    journal_entry_id: Uuid,
    amount: Decimal,
    date: NaiveDate,
}

#[derive(Debug, sqlx::FromRow)]
struct NearMatchCandidateRow {
    stmt_line_id: Uuid,
    journal_entry_id: Uuid,
    amount: Decimal,
    stmt_date: NaiveDate,
    je_date: NaiveDate,
    stmt_reference: String,
    je_reference: String,
}

#[derive(Debug, sqlx::FromRow)]
struct UnmatchedLineRow {
    id: Uuid,
    entity_id: Uuid,
    description: String,
    #[allow(dead_code)]
    reference: String,
    #[allow(dead_code)]
    debit: Option<Decimal>,
    #[allow(dead_code)]
    credit: Option<Decimal>,
    #[allow(dead_code)]
    value_date: NaiveDate,
}

#[derive(Debug, sqlx::FromRow)]
struct HistoricalCategorisationRow {
    description: String,
    assigned_account: String,
}

#[derive(Debug, sqlx::FromRow)]
struct StatementImportRow {
    #[allow(dead_code)]
    id: Uuid,
    #[allow(dead_code)]
    entity_id: Uuid,
    bank_account_id: Uuid,
    #[allow(dead_code)]
    format: String,
    #[allow(dead_code)]
    filename: Option<String>,
    #[allow(dead_code)]
    line_count: i32,
    #[allow(dead_code)]
    matched_count: i32,
    #[allow(dead_code)]
    unmatched_count: i32,
}
