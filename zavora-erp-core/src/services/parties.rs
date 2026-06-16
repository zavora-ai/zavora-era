use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::parties::*;
use crate::types::AgentOrUserId;

/// Create a customer.
pub async fn create_customer(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateCustomerRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let currency = match req.currency.clone() {
        Some(c) => c,
        None => engine.config_for(entity_id).await?.base_currency.clone(),
    };
    let payment_terms = req.payment_terms.unwrap_or(crate::types::PaymentTerms::Net30);
    let ar_account = req.ar_account.unwrap_or_else(|| "1200".to_string());
    let reminder_policy = req.reminder_policy.unwrap_or_default();

    sqlx::query(
        r#"INSERT INTO customers 
           (id, entity_id, name, kra_pin, vat_number, email, phone, address, currency, payment_terms,
            credit_limit, ar_account, reminder_policy, portal_enabled, notes, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, true, $16)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.name)
    .bind(&req.kra_pin)
    .bind(&req.vat_number)
    .bind(serde_json::to_value(&req.email).unwrap_or_default())
    .bind(serde_json::to_value(&req.phone).unwrap_or_default())
    .bind(serde_json::to_value(&req.address).ok())
    .bind(&currency)
    .bind(serde_json::to_string(&payment_terms).unwrap_or_default())
    .bind(req.credit_limit)
    .bind(&ar_account)
    .bind(serde_json::to_value(&reminder_policy).unwrap_or_default())
    .bind(req.portal_enabled.unwrap_or(false))
    .bind(&req.notes)
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}

/// Create a vendor.
pub async fn create_vendor(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateVendorRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let currency = match req.currency.clone() {
        Some(c) => c,
        None => engine.config_for(entity_id).await?.base_currency.clone(),
    };
    let payment_terms = req.payment_terms.unwrap_or(crate::types::PaymentTerms::Net30);
    let ap_account = req.ap_account.unwrap_or_else(|| "3010".to_string());

    sqlx::query(
        r#"INSERT INTO vendors 
           (id, entity_id, name, kra_pin, vat_number, email, phone, address, currency, payment_terms,
            wht_category, resident, ap_account, default_expense_account, bank_details, notes, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, true, $17)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.name)
    .bind(&req.kra_pin)
    .bind(&req.vat_number)
    .bind(serde_json::to_value(&req.email).unwrap_or_default())
    .bind(serde_json::to_value(&req.phone).unwrap_or_default())
    .bind(serde_json::to_value(&req.address).ok())
    .bind(&currency)
    .bind(serde_json::to_string(&payment_terms).unwrap_or_default())
    .bind(req.wht_category.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default()))
    .bind(req.resident.unwrap_or(true))
    .bind(&ap_account)
    .bind(&req.default_expense_account)
    .bind(serde_json::to_value(&req.bank_details).ok())
    .bind(&req.notes)
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}

/// Create an employee.
pub async fn create_employee(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateEmployeeRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let tax_relief = req.tax_relief.unwrap_or(rust_decimal_macros::dec!(2400));

    sqlx::query(
        r#"INSERT INTO employees 
           (id, entity_id, staff_number, full_name, kra_pin, nssf_number, nhif_number, helb_deduction,
            employment_type, basic_salary, allowances, bank_account, tax_relief, disability_exemption, start_date, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, true, $16)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.staff_number)
    .bind(&req.full_name)
    .bind(&req.kra_pin)
    .bind(&req.nssf_number)
    .bind(&req.nhif_number)
    .bind(req.helb_deduction)
    .bind(serde_json::to_string(&req.employment_type).unwrap_or_default())
    .bind(req.basic_salary)
    .bind(serde_json::to_value(&req.allowances).unwrap_or_default())
    .bind(serde_json::to_value(&req.bank_account).unwrap_or_default())
    .bind(tax_relief)
    .bind(req.disability_exemption.unwrap_or(false))
    .bind(req.start_date)
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}
