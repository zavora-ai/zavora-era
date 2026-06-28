# Posting Setup (GL Account Determination)

Zavora ERP resolves the GL accounts used by automatic postings (invoices, bills,
payments, payroll, FX, year-end close) through a per-entity **posting setup**
instead of hardcoded account codes. This is the foundation for the posting-group
model (Business Central / SAP style) that later phases build on.

## How it works

- `PostingSetup` (`zavora-erp-core/src/posting/mod.rs`) holds the account code for
  each accounting role: control accounts (AR/AP), clearing (unapplied payments),
  tax (VAT output/input, WHT), FX (realised/unrealised), equity (retained
  earnings), default income/expense, and the payroll statutory accounts.
- It is stored per entity in `entity_settings.posting_setup` (JSONB). An empty
  object falls back to `PostingSetup::default()`, which mirrors the seeded Kenya
  Standard chart of accounts — so existing entities keep working with no config.
- The engine keeps a live, lock-guarded copy. `engine.posting()` returns the
  current setup; every posting service resolves accounts from it. Saving settings
  updates the database **and** the live copy (`engine.set_posting`), so changes
  take effect immediately without a restart.

## Editing it

UI: **Settings → Posting Accounts**. Each role is a dropdown populated from the
active, non-control accounts in your chart of accounts. Save writes the whole
setup. Accounts that are not in the chart of accounts are flagged in amber so a
mismapping is visible rather than silent.

API:
- `GET /api/v1/settings` → returns the live config including `posting`.
- `PUT /api/v1/settings` with `{ "posting": { ... } }` → persists and reloads.
  Requires Owner/Admin.

## Where accounts are resolved

| Posting path | Accounts resolved from posting setup |
|---|---|
| Invoice post / credit note | `accounts_receivable`, `vat_output`, `default_sales` |
| Bill line default | `default_expense` |
| Payments | `accounts_receivable`, `accounts_payable`, `unapplied_payments`, `wht_payable`, `default_bank` |
| FX (payment) | `realised_fx_gain`, `realised_fx_loss` |
| FX revaluation | `unrealised_fx_gain`, `unrealised_fx_loss` |
| Year-end close | `retained_earnings` |
| Payroll posting | `salaries_expense`, `nssf_employer_expense`, `housing_levy_employer_expense`, `paye_payable`, `nssf_payable`, `sha_payable`, `helb_payable`, `housing_levy_payable`, `net_pay_payable` |

## Known issue surfaced by this work

The default `unapplied_payments` code (`3050`) is **not** present in the seeded
Kenya CoA — a pre-existing mismapping that was previously buried in code. It is
preserved as the default to keep behaviour identical, but it is now visible and
correctable in the Posting Accounts screen. Recommended targets: `1700`
(Unapplied Customer Payments) / `9100`, and `3600` / `9110` for vendor credits.
Splitting customer vs vendor unapplied accounts is part of Phase 2.

> **Posting groups build on this.** The dimension-aware layer (business/product/VAT
> groups → revenue, COGS, A/R, A/P, VAT output/input by trade context) is documented
> in **[POSTING_GROUPS.md](./POSTING_GROUPS.md)**. This flat setup remains the
> guaranteed fallback whenever a group account is unset.

## Roadmap

- **Phase 1 (done):** flat posting setup + resolver wired into all posting paths.
- **Phase 3 (done):** editable in Settings → Posting Accounts, live reload.
- **Phase 2 (done):** posting-group dimensions (Customer / Vendor / Product /
  VAT Business / VAT Product) resolved through setup matrices, so accounts vary
  by trade context (local/export) and item category, including VAT rate +
  output/input account per VAT combination — see
  [POSTING_GROUPS.md](./POSTING_GROUPS.md).
- **Phase 4:** migrate master-record account fields (`customer.ar_account`,
  `vendor.ap_account`, `product.sales_account/purchase_account`, inventory GL
  accounts) onto posting-group references with backward compatibility.
