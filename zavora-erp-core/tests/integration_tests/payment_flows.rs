//! Integration tests for the payment recording flow (Requirement 4.2).
//!
//! Covers the four mandated paths — single (full) payment, partial payment,
//! overpayment, and multi-currency payment — against a live PostgreSQL + Redis
//! via the shared [`crate::common::TestHarness`]. Each test provisions an
//! isolated tenant, builds a posted invoice to pay against, records a payment
//! through `services::payments::record_payment`, and verifies both the invoice
//! balance/status transitions and that **every** posted journal entry balances
//! (sum of `functional_debit` == sum of `functional_credit`).
//!
//! Tests skip gracefully when infrastructure is unavailable (`TestHarness::try_new`
//! returns `None`), matching the established convention in this suite.
//!
//! Service signatures targeted (multi-tenant, post-4.1):
//! - `services::parties::create_customer(engine, entity_id, req, &created_by) -> Uuid`
//! - `services::invoicing::create_invoice(engine, entity_id, req, &created_by) -> Invoice`
//! - `services::invoicing::post_invoice(engine, entity_id, invoice_id, &posted_by) -> Uuid`
//! - `services::payments::record_payment(engine, entity_id, req, &recorded_by) -> Payment`

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use zavora_erp_core::invoicing::{CreateInvoiceLineRequest, CreateInvoiceRequest};
use zavora_erp_core::parties::CreateCustomerRequest;
use zavora_erp_core::payments::{
    PaymentApplicationRequest, PaymentMethod, PaymentType, RecordPaymentRequest,
};
use zavora_erp_core::services::{invoicing, parties, payments};
use zavora_erp_core::types::{AgentOrUserId, VatTreatment};
use zavora_erp_core::ErpEngine;

use crate::common::TestHarness;

/// A deterministic actor for recording the test's documents/payments.
fn actor() -> AgentOrUserId {
    AgentOrUserId::User(Uuid::new_v4())
}

/// Create a customer in the given currency (or the tenant base currency when `None`).
async fn make_customer(h: &TestHarness, currency: Option<&str>) -> Uuid {
    let req = CreateCustomerRequest {
        name: format!("Test Customer {}", Uuid::new_v4()),
        kra_pin: None,
        vat_number: None,
        email: Vec::new(),
        phone: Vec::new(),
        address: None,
        currency: currency.map(|c| c.to_string()),
        payment_terms: None,
        credit_limit: None,
        ar_account: None,
        reminder_policy: None,
        portal_enabled: None,
        notes: None,
    };
    parties::create_customer(&h.engine, h.entity_id, req, &actor())
        .await
        .expect("create customer")
}

/// Create and post a single-line invoice, returning `(invoice_id, balance_due)`.
///
/// Uses a zero-rated line so the balance equals `quantity * unit_price` exactly,
/// keeping payment-amount arithmetic in the tests easy to read.
async fn posted_invoice(
    h: &TestHarness,
    customer_id: Uuid,
    unit_price: Decimal,
    currency: Option<&str>,
    fx_rate: Option<Decimal>,
) -> (Uuid, Decimal) {
    let req = CreateInvoiceRequest {
        customer_id,
        issue_date: Some(h.today),
        due_date: None,
        currency: currency.map(|c| c.to_string()),
        fx_rate,
        lines: vec![CreateInvoiceLineRequest {
            product_id: None,
            description: Some("Consulting services".to_string()),
            quantity: dec!(1),
            unit_price: Some(unit_price),
            discount_percent: None,
            account_code: Some("4000".to_string()),
            vat_treatment: Some(VatTreatment::ZeroRated),
            dimensions: None,
        }],
        template_id: None,
        notes: None,
        send_immediately: None,
    };

    let invoice = invoicing::create_invoice(&h.engine, h.entity_id, req, &actor())
        .await
        .expect("create invoice");

    invoicing::post_invoice(&h.engine, h.entity_id, invoice.id, &actor())
        .await
        .expect("post invoice");

    let balance = invoice_balance(h, invoice.id).await;
    (invoice.id, balance)
}

/// Fetch the current `balance_due` for an invoice.
async fn invoice_balance(h: &TestHarness, invoice_id: Uuid) -> Decimal {
    sqlx::query_scalar::<_, Decimal>("SELECT balance_due FROM invoices WHERE id = $1")
        .bind(invoice_id)
        .fetch_one(&h.pool)
        .await
        .expect("fetch invoice balance_due")
}

/// Fetch the current `status` for an invoice.
async fn invoice_status(h: &TestHarness, invoice_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT status FROM invoices WHERE id = $1")
        .bind(invoice_id)
        .fetch_one(&h.pool)
        .await
        .expect("fetch invoice status")
}

/// Assert that a single journal entry balances in the functional currency.
async fn assert_entry_balances(engine: &ErpEngine, entry_id: Uuid) {
    let (debit, credit): (Decimal, Decimal) = sqlx::query_as(
        "SELECT COALESCE(SUM(functional_debit), 0), COALESCE(SUM(functional_credit), 0) \
         FROM journal_lines WHERE entry_id = $1",
    )
    .bind(entry_id)
    .fetch_one(engine.pool())
    .await
    .expect("sum journal lines");

    assert!(debit > Decimal::ZERO, "entry {entry_id} should have postings");
    assert_eq!(
        debit, credit,
        "journal entry {entry_id} must balance (functional debits == credits)"
    );
}

/// Assert that EVERY journal entry posted for this tenant balances. Because each
/// harness is isolated by `entity_id`, this covers the invoice-posting entry, the
/// payment entry, and any FX gain/loss entry produced along the way.
async fn assert_all_entries_balance(h: &TestHarness) {
    let entry_ids: Vec<Uuid> =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM journal_entries WHERE entity_id = $1")
            .bind(h.entity_id)
            .fetch_all(&h.pool)
            .await
            .expect("fetch entity journal entries");

    assert!(
        !entry_ids.is_empty(),
        "expected at least one posted journal entry for the tenant"
    );

    for id in entry_ids {
        assert_entry_balances(&h.engine, id).await;
    }
}

/// Build a customer payment request that applies the full `amount` to one invoice.
fn customer_payment(
    party_id: Uuid,
    invoice_id: Uuid,
    amount: Decimal,
    currency: Option<&str>,
    fx_rate: Option<Decimal>,
) -> RecordPaymentRequest {
    RecordPaymentRequest {
        payment_type: PaymentType::CustomerPayment,
        party_id,
        payment_date: None,
        amount,
        currency: currency.map(|c| c.to_string()),
        fx_rate,
        method: PaymentMethod::Cash,
        reference: None,
        bank_account_id: None,
        applications: vec![PaymentApplicationRequest {
            document_id: invoice_id,
            amount,
        }],
    }
}

/// Single payment that fully settles an invoice: balance goes to 0, status `paid`,
/// and the payment's journal entry balances.
#[tokio::test]
async fn single_payment_fully_pays_invoice() {
    let Some(h) = TestHarness::try_new().await else {
        return;
    };

    let customer = make_customer(&h, None).await;
    let (invoice_id, balance) = posted_invoice(&h, customer, dec!(1000.00), None, None).await;
    assert_eq!(balance, dec!(1000.00), "zero-rated invoice balance");

    let payment = payments::record_payment(
        &h.engine,
        h.entity_id,
        customer_payment(customer, invoice_id, balance, None, None),
        &actor(),
    )
    .await
    .expect("record full payment");

    assert_eq!(
        invoice_balance(&h, invoice_id).await,
        Decimal::ZERO,
        "invoice should be fully settled"
    );
    assert_eq!(invoice_status(&h, invoice_id).await, "paid");
    assert_eq!(payment.unapplied, Decimal::ZERO, "nothing left unapplied");

    let je = payment.journal_entry_id.expect("payment must have a journal entry");
    assert_entry_balances(&h.engine, je).await;
    assert_all_entries_balance(&h).await;

    h.cleanup().await;
}

/// Partial payment leaves a reduced balance and `partially_paid` status, and the
/// payment's journal entry balances.
#[tokio::test]
async fn partial_payment_leaves_balance() {
    let Some(h) = TestHarness::try_new().await else {
        return;
    };

    let customer = make_customer(&h, None).await;
    let (invoice_id, balance) = posted_invoice(&h, customer, dec!(1000.00), None, None).await;

    let part = dec!(400.00);
    let payment = payments::record_payment(
        &h.engine,
        h.entity_id,
        customer_payment(customer, invoice_id, part, None, None),
        &actor(),
    )
    .await
    .expect("record partial payment");

    assert_eq!(
        invoice_balance(&h, invoice_id).await,
        balance - part,
        "balance_due should drop by the paid amount"
    );
    assert_eq!(invoice_status(&h, invoice_id).await, "partially_paid");
    assert_eq!(payment.unapplied, Decimal::ZERO, "full amount applied to invoice");

    let je = payment.journal_entry_id.expect("payment must have a journal entry");
    assert_entry_balances(&h.engine, je).await;
    assert_all_entries_balance(&h).await;

    h.cleanup().await;
}

/// Overpayment fully settles the invoice and books the excess as unapplied credit;
/// the journal entry (DR Bank / CR AR applied / CR Unapplied excess) balances.
#[tokio::test]
async fn overpayment_creates_unapplied_credit() {
    let Some(h) = TestHarness::try_new().await else {
        return;
    };

    let customer = make_customer(&h, None).await;
    let (invoice_id, balance) = posted_invoice(&h, customer, dec!(1000.00), None, None).await;

    let overpay = balance + dec!(250.00);
    let payment = payments::record_payment(
        &h.engine,
        h.entity_id,
        customer_payment(customer, invoice_id, overpay, None, None),
        &actor(),
    )
    .await
    .expect("record overpayment");

    assert_eq!(
        invoice_balance(&h, invoice_id).await,
        Decimal::ZERO,
        "invoice should be fully settled by the overpayment"
    );
    assert_eq!(invoice_status(&h, invoice_id).await, "paid");
    assert_eq!(
        payment.unapplied,
        dec!(250.00),
        "excess over the invoice balance becomes unapplied credit"
    );

    let je = payment.journal_entry_id.expect("payment must have a journal entry");
    assert_entry_balances(&h.engine, je).await;
    assert_all_entries_balance(&h).await;

    h.cleanup().await;
}

/// Multi-currency payment: invoice issued in USD at one FX rate, paid at a
/// different FX rate. The functional-currency journal entries (the payment entry
/// and the realised FX gain/loss entry) must each balance.
#[tokio::test]
async fn multi_currency_payment_balances() {
    let Some(h) = TestHarness::try_new().await else {
        return;
    };

    // Customer and invoice in USD; invoice booked at 130 KES/USD.
    let customer = make_customer(&h, Some("USD")).await;
    let invoice_rate = dec!(130.00);
    let (invoice_id, balance) =
        posted_invoice(&h, customer, dec!(500.00), Some("USD"), Some(invoice_rate)).await;
    assert_eq!(balance, dec!(500.00), "USD invoice balance in document currency");

    // Pay the full USD balance, but at a higher rate (135 KES/USD) — this triggers
    // a realised FX gain entry in addition to the payment entry.
    let payment_rate = dec!(135.00);
    let payment = payments::record_payment(
        &h.engine,
        h.entity_id,
        customer_payment(customer, invoice_id, balance, Some("USD"), Some(payment_rate)),
        &actor(),
    )
    .await
    .expect("record multi-currency payment");

    assert_eq!(
        invoice_balance(&h, invoice_id).await,
        Decimal::ZERO,
        "USD invoice should be fully paid"
    );
    assert_eq!(invoice_status(&h, invoice_id).await, "paid");

    // The payment's own entry balances in functional currency...
    let je = payment.journal_entry_id.expect("payment must have a journal entry");
    assert_entry_balances(&h.engine, je).await;

    // ...and so does every entry posted along this path, including the FX entry.
    assert_all_entries_balance(&h).await;

    // Sanity: more than one entry was posted (invoice posting + payment + FX).
    let entry_count: i64 =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM journal_entries WHERE entity_id = $1")
            .bind(h.entity_id)
            .fetch_one(&h.pool)
            .await
            .expect("count entries");
    assert!(
        entry_count >= 3,
        "expected invoice, payment, and FX entries; found {entry_count}"
    );

    h.cleanup().await;
}
