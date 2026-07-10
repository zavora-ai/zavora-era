//! Amos's system instruction, assembled from externalized configuration:
//! `system.md` (template) + `AGENTS.md` (operating rules) + the skills catalog.

use crate::config;
use crate::state::AppState;

pub fn system_instruction(state: &AppState) -> String {
    config::system_template()
        .replace("{ui_url}", &state.erp_ui_url)
        .replace("{skills_catalog}", &state.skills.catalog_block())
        .replace("{agents_rules}", &config::agents_rules())
}

/// Build the `{company_name}` + `{company_context}` substitutions from the ERP
/// `/settings` payload so Amos speaks about the ACTUAL tenant it serves instead
/// of a hardcoded "Zavora Technologies Ltd". Every field falls back to neutral
/// wording when absent (e.g. settings unreachable → `serde_json::Value::Null`),
/// so a persona is always produced and no `{placeholder}` ever leaks.
pub fn company_context(settings: &serde_json::Value) -> (String, String) {
    let branding = settings.get("branding");
    let name = branding
        .and_then(|b| b.get("company_name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("the business")
        .to_string();

    let currency = settings
        .get("base_currency")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("KES");

    let vat_registered = settings
        .get("tax_config")
        .and_then(|t| t.get("vat_registered"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let fy = settings.get("fiscal_year_end");
    let fy_month = fy.and_then(|f| f.get("month")).and_then(|v| v.as_u64()).unwrap_or(12);
    let fy_day = fy.and_then(|f| f.get("day")).and_then(|v| v.as_u64()).unwrap_or(31);
    let month_name = match fy_month {
        1 => "January", 2 => "February", 3 => "March", 4 => "April",
        5 => "May", 6 => "June", 7 => "July", 8 => "August",
        9 => "September", 10 => "October", 11 => "November", _ => "December",
    };

    let address = branding
        .and_then(|b| b.get("address"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let kra_pin = branding
        .and_then(|b| b.get("kra_pin"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());

    let mut lines: Vec<String> = Vec::new();
    let loc = address.map(|a| format!("{a}. ")).unwrap_or_default();
    lines.push(format!("- {name}. {loc}Functional currency: {currency}."));
    if vat_registered {
        lines.push(
            "- VAT-registered: charge output VAT on sales and reclaim input VAT on purchases; \
             the VAT return nets the two. Customers may still withhold WHT on some services — \
             that is a tax credit (WHT receivable), not lost income."
                .to_string(),
        );
    } else {
        lines.push(
            "- Not VAT-registered (VAT on purchases is booked as part of the cost). Customers \
             sometimes withhold 5% WHT on consultancy fees — that becomes a tax credit \
             (WHT receivable), not lost income."
                .to_string(),
        );
    }
    if let Some(pin) = kra_pin {
        lines.push(format!("- KRA PIN: {pin}."));
    }
    lines.push(format!(
        "- Foreign-currency amounts (USD, EUR) always matter in both the original currency and {currency}."
    ));
    lines.push(format!(
        "- The books run on Zavora ERA, the company's own ERP. The fiscal year ends {month_name} {fy_day} — \
         use the current date above to reason about which financial year the books are in."
    ));

    (name, lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn falls_back_when_settings_absent() {
        let (name, ctx) = company_context(&serde_json::Value::Null);
        assert_eq!(name, "the business");
        assert!(ctx.contains("Functional currency: KES"));
        assert!(ctx.contains("Not VAT-registered"));
        assert!(ctx.contains("fiscal year ends December 31"));
        assert!(!ctx.contains("{"), "no placeholder may leak");
    }

    #[test]
    fn uses_real_company_facts() {
        let settings = json!({
            "base_currency": "USD",
            "branding": { "company_name": "Acme Traders Ltd", "kra_pin": "P051234567M", "address": "Nairobi" },
            "tax_config": { "vat_registered": true },
            "fiscal_year_end": { "month": 6, "day": 30 }
        });
        let (name, ctx) = company_context(&settings);
        assert_eq!(name, "Acme Traders Ltd");
        assert!(ctx.contains("Acme Traders Ltd. Nairobi. Functional currency: USD."));
        assert!(ctx.contains("VAT-registered: charge output VAT"));
        assert!(ctx.contains("KRA PIN: P051234567M"));
        assert!(ctx.contains("fiscal year ends June 30"));
    }
}
