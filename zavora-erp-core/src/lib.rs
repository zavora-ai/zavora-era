//! Zavora ERP — Core ERP Engine
//!
//! A double-entry accounting engine with Kenya-specific compliance features
//! (KRA iTax, M-Pesa, PAYE/NSSF/NHIF, WHT) designed for both human UI
//! and agentic layer consumption.
//!
//! # Architecture
//!
//! The engine is structured as a library crate with no runtime dependency.
//! All public methods are async — the caller provides the runtime context.
//!
//! # Module Layout
//!
//! - `ledger` — Chart of Accounts, Journal entries, GL
//! - `parties` — Customer, Vendor, Employee entities
//! - `catalog` — Products & Services
//! - `invoicing` — Invoices, Estimates, Recurring, Credit Notes
//! - `ap` — Bills, Supplier Credit Notes
//! - `payments` — Online payments, M-Pesa, receipts, partial pay
//! - `transactions` — Categorisation queue, split, merge
//! - `bank` — Bank feeds, reconciliation
//! - `payroll` — Employees, pay runs, Kenya statutory
//! - `period` — Fiscal periods
//! - `tax` — VAT, WHT, PAYE, NSSF, NHIF
//! - `fx` — Exchange rates, revaluation
//! - `assets` — Fixed assets, depreciation
//! - `inventory` — Stock, FIFO/WAC
//! - `reporting` — All report types, export
//! - `notifications` — Reminders, webhooks, push
//! - `documents` — Attachments, templates, branding
//! - `rbac` — Users, roles, permissions
//! - `settings` — Entity config, sequences, branding
//! - `audit` — AuditEvent, Redis stream

pub mod error;
pub mod types;
pub mod engine;

// Domain modules
pub mod ledger;
pub mod parties;
pub mod catalog;
pub mod invoicing;
pub mod ap;
pub mod payments;
pub mod transactions;
pub mod bank;
pub mod payroll;
pub mod period;
pub mod tax;
pub mod fx;
pub mod assets;
pub mod inventory;
pub mod reporting;
pub mod notifications;
pub mod documents;
pub mod rbac;
pub mod settings;
pub mod audit;
pub mod tenant;
pub mod posting;
pub mod auth;
pub mod money;

// Service layer — business logic implementations
pub mod services;

// Re-export primary public types
pub use engine::{ErpEngine, AgentPostingResult, PostingRequest};
pub use error::{ErpError, ErpResult};
pub use settings::ErpConfig;
pub use posting::PostingSetup;
pub use types::AgentOrUserId;
pub use money::{round_money, round_paye};
