//! Property-based test suite for `zavora-erp-core`.
//!
//! This is the crate root for all `proptest`-based property tests. Individual
//! property test modules live under `tests/property_tests/` and are declared
//! here. They share the test harness in `tests/common/mod.rs`.
//!
//! Add new property test files as `tests/property_tests/<name>.rs` and declare
//! them with `mod <name>;` below. DB-backed property tests should acquire a
//! tenant via `crate::common::TestHarness::try_new()` and skip when `None`.

#[path = "common/mod.rs"]
mod common;

#[path = "property_tests/harness_smoke.rs"]
mod harness_smoke;
