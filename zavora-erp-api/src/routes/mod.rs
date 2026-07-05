pub mod dashboard;
pub mod accounts;
pub mod periods;
pub mod journal;
pub mod parties;
pub mod catalog;
pub mod invoices;
pub mod estimates;
pub mod invoice_templates;
pub mod bills;
pub mod budgets;
pub mod consolidation;
pub mod custom_reports;
pub mod dimensions;
pub mod notifications;
pub mod onboarding;
pub mod pagination;
pub mod reconciliation;
pub mod recurring_journals;
pub mod tax_filings;
pub mod wht;
pub mod report_schedules;
pub mod supplier_credit_notes;
pub mod payments;
pub mod posting_groups;
pub mod transactions;
pub mod bank;
pub mod payroll;
pub mod leave;
pub mod staff_auth;
pub mod inventory;
pub mod assets;
pub mod fx;
pub mod audit;
pub mod reports;
pub mod agent;
pub mod receipts;
pub mod attachments;
pub mod ocr_provider;
pub mod pdf_text;
pub mod settings;
pub mod users;
pub mod auth_signup;
pub mod auth_tenants;
pub mod procurement;
pub mod portal;
pub mod portal_auth;

use axum::{http::StatusCode, response::IntoResponse, Json};
use zavora_erp_core::ErpError;

/// Convert ErpError to HTTP response.
pub fn err_response(e: ErpError) -> impl IntoResponse {
    let (status, message) = match &e {
        ErpError::NotFound { .. } => (StatusCode::NOT_FOUND, e.to_string()),
        ErpError::ValidationFailed { .. } => (StatusCode::BAD_REQUEST, e.to_string()),
        ErpError::Unbalanced { .. } => (StatusCode::BAD_REQUEST, e.to_string()),
        ErpError::PeriodClosed { .. } => (StatusCode::CONFLICT, e.to_string()),
        ErpError::Duplicate { .. } => (StatusCode::CONFLICT, e.to_string()),
        ErpError::DuplicateReference { .. } => (StatusCode::CONFLICT, e.to_string()),
        ErpError::PermissionDenied { .. } => (StatusCode::FORBIDDEN, e.to_string()),
        ErpError::Unauthorized { .. } => (StatusCode::UNAUTHORIZED, e.to_string()),
        ErpError::InsufficientStock { .. } => (StatusCode::CONFLICT, e.to_string()),
        ErpError::CreditLimitExceeded { .. } => (StatusCode::CONFLICT, e.to_string()),
        ErpError::Overpayment { .. } => (StatusCode::BAD_REQUEST, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    (status, Json(serde_json::json!({ "error": message })))
}
