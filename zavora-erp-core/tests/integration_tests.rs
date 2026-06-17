//! Integration test suite for `zavora-erp-core`.
//!
//! This is the crate root for DB-backed integration tests (payment flows,
//! period close, FX revaluation, settings persistence, etc.). Individual test
//! modules live under `tests/integration_tests/` and are declared here. They
//! share the test harness in `tests/common/mod.rs`.
//!
//! Add new integration test files as `tests/integration_tests/<name>.rs` and
//! declare them with `mod <name>;` below. Tests should acquire a tenant via
//! `crate::common::TestHarness::try_new()` and skip when `None` so the suite
//! degrades gracefully without a database.

#[path = "common/mod.rs"]
mod common;

#[path = "integration_tests/harness_smoke.rs"]
mod harness_smoke;

#[path = "integration_tests/payment_flows.rs"]
mod payment_flows;
