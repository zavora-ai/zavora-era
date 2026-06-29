# UI Gaps & Findings — Onboarding Walkthrough

Findings from driving the app end-to-end (Playwright) while setting up a real
company (Zavora Technologies Ltd — Kenyan services business, not VAT-registered).
Status reflects work done in this session.

Legend: 🔴 high · 🟠 medium · 🟡 low · ✅ fixed this session

---

## ✅ Fixed this session

### 1. Posting Accounts dropdown excluded control accounts 🟠
- **Where:** Settings → Posting Accounts (`PostingAccountsTab.tsx`).
- **Problem:** The account picker filtered out **all** control accounts, but A/R and
  A/P *must* map to control accounts (Trade Debtors / Trade Creditors). They rendered
  as "1200 — (not in chart of accounts)" with an amber error, implying a broken setup
  even though the codes were valid and posted correctly.
- **Why it matters:** Kenyan/IFRS practice posts A/R and A/P to the **control
  account** (Sales/Purchases Ledger Control), backed by per-party subledgers. Excluding
  control accounts is wrong for exactly the two roles that require them.
- **Fix:** Control-account roles (`accounts_receivable`, `accounts_payable`) now
  include control accounts in their picker; other roles still exclude them. AR shows
  "1200 — Trade Debtors", AP "3010 — Trade Creditors".

### 2. Products form charged VAT regardless of registration 🟠
- **Where:** Products → Add Product (`ProductsPage.tsx`).
- **Problem:** New items defaulted to **Standard 16%** even when the company is **not
  VAT-registered**, risking accidental output VAT on every invoice line.
- **Fix:** The tax-treatment default now derives from the company's VAT registration —
  **Exempt** when not registered, **Standard 16%** when registered — and respects a
  manual choice. Backend `TaxConfig` default also set to Exempt for new (non-registered)
  tenants.

### 3. Seed lacked accounts a Kenyan services business needs 🟠
- **Where:** `ledger/coa_template.rs`, `posting/mod.rs`.
- **Problem:** The Kenya Standard COA seeded **WHT Payable** but no **WHT Receivable**,
  so a service provider whose customers withhold 5% had nowhere to book the credit.
  No "unpaid share capital", "royalty income", or "software/cloud/subscriptions" lines.
  Posting defaults pointed sales→Sales Revenue and purchases→COGS (goods-centric).
- **Fix (seed, services-first defaults):** added 1310 WHT Receivable, 1610 Unpaid Share
  Capital, 5250 Royalty Income, 7350 Software/Cloud/Subscriptions; default sales → 5100
  Service Revenue, default purchase → 7350.

### 4. Banking import copy omitted PDF/Excel 🟡
- **Where:** Banking page reconciliation blurb (`BankingPage.tsx`).
- **Problem:** Said "Import statements in MT940, OFX, or CSV format" — missing the new
  **PDF** and **Excel** import paths.
- **Fix:** Now reads "CSV, MT940, OFX, PDF, or Excel (M-Pesa / bank exports)".

### 5. Base currency / fiscal year-end were not editable ✅
- **Where:** Settings → Company (`SettingsPage.tsx`).
- **Problem:** Signup silently defaulted base currency to KES and year-end to December,
  and both were **read-only** in Settings — a non-KES / non-December company couldn't
  correct them.
- **Fix:** Base Currency and Fiscal Year-End are now **editable selects** in Settings →
  Company (the update API already accepted both), with a warning to set them before
  posting. *(Capturing them at signup is a further nice-to-have but is deferred — it
  touches the provisioning transaction and fiscal-period seeding.)*

### 6. Settings → Company missing statutory fields ✅
- **Where:** Settings → Company; `BrandingConfig` (`settings/mod.rs`).
- **Problem:** No company registration number, registered address, or phone — needed on
  invoices and tax correspondence.
- **Fix:** Added **Registration Number**, **Registered Address**, and **Phone** to the
  Company tab (added `registration_number` to `BrandingConfig`; address/phone already
  existed in the backend). Verified they persist (e.g. PVT-Q7UDGDA).

### 7. Non-resident vendors weren't prompted about WHT ✅
- **Where:** Vendors → Tax & WHT (`VendorsPage.tsx`).
- **Problem:** Vendors default to Resident / WHT Category None; foreign suppliers gave no
  prompt that withholding may apply.
- **Fix:** Selecting **Non-Resident** now shows a prompt that payments for
  services/royalties/management/professional fees to non-residents are commonly subject
  to 20% WHT (or a treaty rate), and the rate box emphasises the applicable
  resident/non-resident column.

---

## 🟠 Open — recommended next

### 8. Capture base currency / year-end at signup 🟡
- Now editable in Settings (gap #5), but ideally also selectable on the Create
  Organization form so it's right from the first transaction. Deferred — needs threading
  through `provision_tenant` + fiscal-period seeding.

### 9. Surface company statutory fields on invoice/PDF templates 🟡
- Registration number / address / phone are now captured (gap #6) but should also render
  on the invoice and statement PDF headers.

---

## Notes / non-issues
- "Import Statement" is correctly disabled until a bank account exists.
- Vendor records have no per-vendor currency field — currency is set per bill, which is
  acceptable (and matches multi-currency-per-document behaviour).
- The dashboard onboarding checklist (Set up company → customer → invoice → payment →
  vendor) is present and accurate.

---

## Validated working (no change needed)
- **Bank statement import** — PDF (PDFium local) + Excel (M-Pesa) extraction with
  balance-delta reconciliation; live test parsed the real Equity statement to 33 rows.
- **Tax settings** — VAT Registered toggle (defaults off), WHT enabled.
- **Posting groups** — DOMESTIC business group → A/R 1200 / A/P 3010 control accounts;
  general matrix routes services to 5100.
- **Chart of accounts** — full Kenya Standard template seeds on signup.
