use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Layout style for invoice templates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TemplateLayout {
    Classic,
    Modern,
    Minimal,
}

/// An invoice template controlling branding and layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceTemplate {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub secondary_color: Option<String>,
    pub font: String,
    pub footer_text: Option<String>,
    pub show_bank_details: bool,
    pub show_mpesa_paybill: bool,
    pub layout: TemplateLayout,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

/// Database row for invoice template.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct InvoiceTemplateRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub secondary_color: Option<String>,
    pub font: String,
    pub footer_text: Option<String>,
    pub show_bank_details: bool,
    pub show_mpesa_paybill: bool,
    pub layout: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

/// Request to create an invoice template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub logo_url: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub font: Option<String>,
    pub footer_text: Option<String>,
    pub show_bank_details: Option<bool>,
    pub show_mpesa_paybill: Option<bool>,
    pub layout: Option<TemplateLayout>,
    pub is_default: Option<bool>,
}
