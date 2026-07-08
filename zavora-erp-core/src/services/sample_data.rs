//! Sample-company seeding for new-tenant onboarding.
//!
//! When a user opts in at signup ("explore with sample data"), this populates
//! the fresh tenant with a realistic, coherent Kenyan-SME dataset — customers,
//! vendors, products, and a spread of posted + draft invoices — so the
//! dashboard, AR ageing, P&L, balance sheet and GL are all immediately
//! populated to explore. It reuses the real service layer, so every posted
//! document books correct double-entry through the tenant's posting setup.
//!
//! Best-effort by design: it runs *after* the tenant is provisioned and never
//! blocks or fails signup — a partial seed is logged and tolerated.

use chrono::{Duration, Utc};
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::catalog::{CreateProductRequest, ProductType};
use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::invoicing::invoice::CreateInvoiceRequest;
use crate::invoicing::line::CreateInvoiceLineRequest;
use crate::parties::customer::CreateCustomerRequest;
use crate::parties::vendor::CreateVendorRequest;
use crate::services::{catalog, invoicing, parties};
use crate::types::{AgentOrUserId, PaymentTerms};

/// Counts of what was seeded (for logging).
#[derive(Debug, Default)]
pub struct SampleSeedSummary {
    pub customers: u32,
    pub vendors: u32,
    pub products: u32,
    pub invoices_posted: u32,
    pub invoices_draft: u32,
}

fn cust(name: &str, terms: PaymentTerms) -> CreateCustomerRequest {
    CreateCustomerRequest {
        name: name.to_string(),
        kra_pin: None,
        vat_number: None,
        email: Vec::new(),
        phone: Vec::new(),
        address: None,
        currency: None,
        payment_terms: Some(terms),
        credit_limit: None,
        ar_account: None,
        reminder_policy: None,
        portal_enabled: None,
        notes: Some("Sample data — safe to delete.".to_string()),
    }
}

fn vend(name: &str) -> CreateVendorRequest {
    CreateVendorRequest {
        name: name.to_string(),
        kra_pin: None,
        vat_number: None,
        email: Vec::new(),
        phone: Vec::new(),
        address: None,
        currency: None,
        payment_terms: Some(PaymentTerms::Net30),
        wht_category: None,
        resident: None,
        ap_account: None,
        default_expense_account: None,
        bank_details: None,
        notes: Some("Sample data — safe to delete.".to_string()),
    }
}

fn prod(name: &str, kind: ProductType, price: rust_decimal::Decimal) -> CreateProductRequest {
    CreateProductRequest {
        name: name.to_string(),
        description: Some("Sample item".to_string()),
        product_type: kind,
        unit_price: Some(price),
        currency: None,
        uom: None,
        sales_account: None,
        purchase_account: None,
        vat_treatment: None, // defaults to Standard 16% VAT
        track_inventory: Some(false),
    }
}

/// Seed a realistic sample company into an already-provisioned tenant.
pub async fn seed_sample_company(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<SampleSeedSummary> {
    let by = AgentOrUserId::Agent("sample-data".to_string());
    let mut s = SampleSeedSummary::default();

    // ── Customers ────────────────────────────────────────────────────────────
    let customer_specs = [
        cust("Nairobi Fresh Foods Ltd", PaymentTerms::Net30),
        cust("Mombasa Traders Co.", PaymentTerms::Net14),
        cust("Kisumu Hardware Supplies", PaymentTerms::Net30),
        cust("Rift Valley Distributors", PaymentTerms::Net45),
        cust("Coastal Logistics Ltd", PaymentTerms::Net60),
        cust("Highlands Coffee House", PaymentTerms::DueOnReceipt),
    ];
    let mut customers = Vec::new();
    for c in customer_specs {
        match parties::create_customer(engine, entity_id, c, &by).await {
            Ok(id) => { customers.push(id); s.customers += 1; }
            Err(e) => tracing::warn!("sample seed: customer failed: {e}"),
        }
    }

    // ── Vendors ──────────────────────────────────────────────────────────────
    for v in [
        vend("Kenya Power & Lighting Co."),
        vend("Safaricom PLC"),
        vend("Bidco Africa Ltd"),
        vend("Office Mart Kenya"),
        vend("Nation Media Group"),
    ] {
        match parties::create_vendor(engine, entity_id, v, &by).await {
            Ok(_) => s.vendors += 1,
            Err(e) => tracing::warn!("sample seed: vendor failed: {e}"),
        }
    }

    // ── Products & services ────────────────────────────────────────────────
    let product_specs = [
        prod("Consulting Services (hr)", ProductType::Service, dec!(15000)),
        prod("Software License — Annual", ProductType::Service, dec!(120000)),
        prod("Installation & Setup", ProductType::Service, dec!(8000)),
        prod("Monthly Support Plan", ProductType::Service, dec!(5000)),
        prod("Branded Merchandise", ProductType::Goods, dec!(1800)),
        prod("Maize Flour 2kg", ProductType::Goods, dec!(180)),
        prod("Cooking Oil 5L", ProductType::Goods, dec!(1450)),
        prod("Standard Delivery", ProductType::Service, dec!(2500)),
    ];
    let mut products = Vec::new();
    for p in product_specs {
        match catalog::create_product(engine, entity_id, p, &by).await {
            Ok(id) => { products.push(id); s.products += 1; }
            Err(e) => tracing::warn!("sample seed: product failed: {e}"),
        }
    }

    // ── Invoices (posted + a couple of drafts) ───────────────────────────────
    if customers.is_empty() || products.is_empty() {
        return Ok(s); // nothing to invoice against
    }
    let today = Utc::now().date_naive();
    // (customer_index, [(product_index, qty)], days_ago, post?)
    let invoice_specs: &[(usize, &[(usize, i64)], i64, bool)] = &[
        (0, &[(0, 3), (2, 1)], 75, true),
        (1, &[(1, 1)], 60, true),
        (2, &[(4, 10), (7, 1)], 48, true),
        (3, &[(5, 200), (6, 30)], 33, true),
        (4, &[(3, 1), (2, 2)], 20, true),
        (5, &[(0, 2)], 12, true),
        (1, &[(3, 1)], 6, false),   // draft
        (2, &[(1, 1), (7, 1)], 2, false), // draft
    ];
    for (ci, lines_spec, days_ago, post) in invoice_specs {
        let lines: Vec<CreateInvoiceLineRequest> = lines_spec
            .iter()
            .filter_map(|(pi, qty)| products.get(*pi).map(|pid| CreateInvoiceLineRequest {
                product_id: Some(*pid),
                description: None,
                quantity: rust_decimal::Decimal::from(*qty),
                unit_price: None,
                discount_percent: None,
                account_code: None,
                vat_treatment: None,
                dimensions: None,
            }))
            .collect();
        if lines.is_empty() { continue; }
        let req = CreateInvoiceRequest {
            customer_id: customers[*ci % customers.len()],
            issue_date: Some(today - Duration::days(*days_ago)),
            due_date: None,
            currency: None,
            fx_rate: None,
            lines,
            template_id: None,
            notes: Some("Sample invoice".to_string()),
            send_immediately: None,
        };
        match invoicing::create_invoice(engine, entity_id, req, &by).await {
            Ok(inv) => {
                if *post {
                    match invoicing::post_invoice(engine, entity_id, inv.id, &by).await {
                        Ok(_) => s.invoices_posted += 1,
                        Err(e) => { s.invoices_draft += 1; tracing::warn!("sample seed: post invoice failed: {e}"); }
                    }
                } else {
                    s.invoices_draft += 1;
                }
            }
            Err(e) => tracing::warn!("sample seed: invoice failed: {e}"),
        }
    }

    Ok(s)
}
