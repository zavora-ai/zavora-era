-- Zavora ERA — Initial Database Schema
-- Covers all tables from spec sections 26.1–26.7 plus supporting tables.

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ============================================================
-- ACCOUNTS (Chart of Accounts)
-- ============================================================
CREATE TABLE accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    parent_code TEXT,
    currency CHAR(3),
    is_control BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, code)
);

CREATE INDEX idx_accounts_entity ON accounts(entity_id);
CREATE INDEX idx_accounts_type ON accounts(entity_id, account_type);
CREATE INDEX idx_accounts_parent ON accounts(entity_id, parent_code);

-- ============================================================
-- FISCAL PERIODS
-- ============================================================
CREATE TABLE fiscal_periods (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    name TEXT NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'future',
    fiscal_year INTEGER NOT NULL,
    period_number INTEGER NOT NULL,
    closed_by JSONB,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, start_date)
);

CREATE INDEX idx_periods_entity ON fiscal_periods(entity_id);
CREATE INDEX idx_periods_date ON fiscal_periods(entity_id, start_date, end_date);

-- ============================================================
-- JOURNAL ENTRIES
-- ============================================================
CREATE TABLE journal_entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    number TEXT NOT NULL,
    date DATE NOT NULL,
    period_id UUID NOT NULL REFERENCES fiscal_periods(id),
    source TEXT NOT NULL,
    reference TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft',
    created_by JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    posted_at TIMESTAMPTZ,
    UNIQUE(entity_id, number)
);

CREATE INDEX idx_je_entity_date ON journal_entries(entity_id, date);
CREATE INDEX idx_je_entity_status ON journal_entries(entity_id, status);
CREATE INDEX idx_je_reference ON journal_entries(entity_id, reference);

CREATE TABLE journal_lines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entry_id UUID NOT NULL REFERENCES journal_entries(id),
    account_code TEXT NOT NULL,
    debit NUMERIC,
    credit NUMERIC,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    fx_rate NUMERIC NOT NULL DEFAULT 1,
    functional_debit NUMERIC,
    functional_credit NUMERIC,
    description TEXT,
    dimensions JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX idx_jl_entry ON journal_lines(entry_id);
CREATE INDEX idx_jl_account ON journal_lines(account_code);

-- ============================================================
-- ENTITY SETTINGS
-- ============================================================
CREATE TABLE entity_settings (
    entity_id UUID PRIMARY KEY,
    base_currency CHAR(3) NOT NULL DEFAULT 'KES',
    fiscal_year_end TEXT NOT NULL DEFAULT '{"month":12,"day":31}',
    coa_template TEXT NOT NULL DEFAULT 'KenyaStandard',
    branding JSONB NOT NULL DEFAULT '{}'::jsonb,
    sequences JSONB NOT NULL DEFAULT '{"invoice_prefix":"INV","invoice_next":1,"estimate_prefix":"EST","estimate_next":1,"credit_note_prefix":"CN","credit_note_next":1,"bill_prefix":"BILL","bill_next":1,"journal_prefix":"JE","journal_next":1,"payment_prefix":"PAY","payment_next":1,"year_reset":true}'::jsonb,
    tax_config JSONB NOT NULL DEFAULT '{"vat_registered":false,"standard_vat_rate":"0.16","default_vat_treatment":"Standard16","wht_enabled":true,"paye_enabled":true,"vat_period":"Monthly"}'::jsonb,
    payment_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by UUID
);

-- ============================================================
-- CUSTOMERS
-- ============================================================
CREATE TABLE customers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    name TEXT NOT NULL,
    kra_pin TEXT,
    vat_number TEXT,
    email JSONB NOT NULL DEFAULT '[]'::jsonb,
    phone JSONB NOT NULL DEFAULT '[]'::jsonb,
    address JSONB,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    payment_terms TEXT NOT NULL DEFAULT 'Net30',
    credit_limit NUMERIC,
    ar_account TEXT NOT NULL DEFAULT '1200',
    reminder_policy JSONB NOT NULL DEFAULT '{"reminders":[]}'::jsonb,
    portal_enabled BOOLEAN NOT NULL DEFAULT false,
    notes TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_customers_entity ON customers(entity_id);
CREATE INDEX idx_customers_name ON customers(entity_id, name);

-- ============================================================
-- VENDORS
-- ============================================================
CREATE TABLE vendors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    name TEXT NOT NULL,
    kra_pin TEXT,
    vat_number TEXT,
    email JSONB NOT NULL DEFAULT '[]'::jsonb,
    phone JSONB NOT NULL DEFAULT '[]'::jsonb,
    address JSONB,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    payment_terms TEXT NOT NULL DEFAULT 'Net30',
    wht_category TEXT,
    resident BOOLEAN NOT NULL DEFAULT true,
    ap_account TEXT NOT NULL DEFAULT '3010',
    default_expense_account TEXT,
    bank_details JSONB,
    notes TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_vendors_entity ON vendors(entity_id);
CREATE INDEX idx_vendors_name ON vendors(entity_id, name);

-- ============================================================
-- EMPLOYEES
-- ============================================================
CREATE TABLE employees (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    staff_number TEXT NOT NULL,
    full_name TEXT NOT NULL,
    kra_pin TEXT NOT NULL,
    nssf_number TEXT,
    nhif_number TEXT,
    helb_deduction NUMERIC,
    employment_type TEXT NOT NULL DEFAULT 'Permanent',
    basic_salary NUMERIC NOT NULL,
    allowances JSONB NOT NULL DEFAULT '[]'::jsonb,
    bank_account JSONB NOT NULL DEFAULT '{}'::jsonb,
    tax_relief NUMERIC NOT NULL DEFAULT 2400,
    disability_exemption BOOLEAN NOT NULL DEFAULT false,
    start_date DATE NOT NULL,
    end_date DATE,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, staff_number)
);

CREATE INDEX idx_employees_entity ON employees(entity_id);

-- ============================================================
-- PRODUCTS & SERVICES
-- ============================================================
CREATE TABLE products (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    product_type TEXT NOT NULL DEFAULT 'Service',
    unit_price NUMERIC,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    uom TEXT NOT NULL DEFAULT 'Each',
    sales_account TEXT NOT NULL DEFAULT '5000',
    purchase_account TEXT NOT NULL DEFAULT '6000',
    vat_treatment TEXT NOT NULL DEFAULT 'Standard16',
    track_inventory BOOLEAN NOT NULL DEFAULT false,
    inventory_item_id UUID,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_products_entity ON products(entity_id);

-- ============================================================
-- INVOICES
-- ============================================================
CREATE TABLE invoices (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    number TEXT NOT NULL,
    invoice_type TEXT NOT NULL DEFAULT 'invoice',
    customer_id UUID NOT NULL REFERENCES customers(id),
    issue_date DATE NOT NULL,
    due_date DATE NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    fx_rate NUMERIC NOT NULL DEFAULT 1,
    subtotal NUMERIC NOT NULL DEFAULT 0,
    discount_total NUMERIC NOT NULL DEFAULT 0,
    tax_total NUMERIC NOT NULL DEFAULT 0,
    gross_total NUMERIC NOT NULL DEFAULT 0,
    amount_paid NUMERIC NOT NULL DEFAULT 0,
    balance_due NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    source_estimate UUID,
    credit_note_for UUID,
    journal_entry_id UUID,
    sent_at TIMESTAMPTZ,
    viewed_at TIMESTAMPTZ,
    paid_at TIMESTAMPTZ,
    template_id UUID,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, number)
);

CREATE INDEX idx_invoices_entity ON invoices(entity_id);
CREATE INDEX idx_invoices_customer ON invoices(customer_id);
CREATE INDEX idx_invoices_status ON invoices(entity_id, status);
CREATE INDEX idx_invoices_due ON invoices(entity_id, due_date) WHERE status NOT IN ('paid', 'voided');

CREATE TABLE invoice_lines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    invoice_id UUID NOT NULL REFERENCES invoices(id),
    product_id UUID REFERENCES products(id),
    description TEXT NOT NULL DEFAULT '',
    quantity NUMERIC NOT NULL DEFAULT 1,
    unit_price NUMERIC NOT NULL DEFAULT 0,
    discount_percent NUMERIC NOT NULL DEFAULT 0,
    account_code TEXT NOT NULL,
    vat_treatment TEXT NOT NULL DEFAULT 'Standard16',
    line_total NUMERIC NOT NULL DEFAULT 0,
    vat_amount NUMERIC NOT NULL DEFAULT 0
);

CREATE INDEX idx_invoice_lines_invoice ON invoice_lines(invoice_id);

-- ============================================================
-- ESTIMATES
-- ============================================================
CREATE TABLE estimates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    number TEXT NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(id),
    issue_date DATE NOT NULL,
    expiry_date DATE NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    fx_rate NUMERIC NOT NULL DEFAULT 1,
    subtotal NUMERIC NOT NULL DEFAULT 0,
    tax_total NUMERIC NOT NULL DEFAULT 0,
    gross_total NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    converted_to UUID,
    notes TEXT,
    template_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, number)
);

-- ============================================================
-- RECURRING INVOICES
-- ============================================================
CREATE TABLE recurring_invoices (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(id),
    template JSONB NOT NULL,
    frequency TEXT NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE,
    next_run DATE NOT NULL,
    auto_send BOOLEAN NOT NULL DEFAULT false,
    auto_charge BOOLEAN NOT NULL DEFAULT false,
    last_run DATE,
    run_count INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- BILLS (Accounts Payable)
-- ============================================================
CREATE TABLE bills (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    number TEXT NOT NULL,
    vendor_id UUID NOT NULL REFERENCES vendors(id),
    vendor_invoice_number TEXT,
    issue_date DATE NOT NULL,
    due_date DATE NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    fx_rate NUMERIC NOT NULL DEFAULT 1,
    subtotal NUMERIC NOT NULL DEFAULT 0,
    tax_total NUMERIC NOT NULL DEFAULT 0,
    wht_amount NUMERIC NOT NULL DEFAULT 0,
    gross_total NUMERIC NOT NULL DEFAULT 0,
    amount_paid NUMERIC NOT NULL DEFAULT 0,
    balance_due NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    journal_entry_id UUID,
    approved_by UUID,
    approved_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, number)
);

CREATE INDEX idx_bills_entity ON bills(entity_id);
CREATE INDEX idx_bills_vendor ON bills(vendor_id);
CREATE INDEX idx_bills_status ON bills(entity_id, status);

-- ============================================================
-- PAYMENTS
-- ============================================================
CREATE TABLE payments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    number TEXT NOT NULL,
    payment_type TEXT NOT NULL,
    party_id UUID NOT NULL,
    payment_date DATE NOT NULL,
    amount NUMERIC NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    fx_rate NUMERIC NOT NULL DEFAULT 1,
    method JSONB NOT NULL,
    reference TEXT NOT NULL DEFAULT '',
    bank_account_id UUID,
    applications JSONB NOT NULL DEFAULT '[]'::jsonb,
    unapplied NUMERIC NOT NULL DEFAULT 0,
    journal_entry_id UUID,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, number)
);

CREATE INDEX idx_payments_entity ON payments(entity_id);
CREATE INDEX idx_payments_party ON payments(party_id);

-- ============================================================
-- IMPORTED TRANSACTIONS (Bank feed / categorisation queue)
-- ============================================================
CREATE TABLE imported_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    bank_account UUID NOT NULL,
    value_date DATE NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    reference TEXT NOT NULL DEFAULT '',
    debit NUMERIC,
    credit NUMERIC,
    running_bal NUMERIC NOT NULL DEFAULT 0,
    category_status TEXT NOT NULL DEFAULT 'uncategorised',
    assigned_account TEXT,
    merged_into UUID,
    journal_entry_id UUID,
    suggestion JSONB,
    import_batch_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_txn_entity ON imported_transactions(entity_id);
CREATE INDEX idx_txn_status ON imported_transactions(entity_id, category_status);
CREATE INDEX idx_txn_bank ON imported_transactions(bank_account);

-- ============================================================
-- BANK ACCOUNTS
-- ============================================================
CREATE TABLE bank_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    name TEXT NOT NULL,
    bank_name TEXT NOT NULL,
    account_number TEXT NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    gl_account TEXT NOT NULL,
    feed_enabled BOOLEAN NOT NULL DEFAULT false,
    feed_provider TEXT,
    last_sync TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- PAY RUNS
-- ============================================================
CREATE TABLE pay_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    period_id UUID NOT NULL REFERENCES fiscal_periods(id),
    pay_date DATE NOT NULL,
    total_gross NUMERIC NOT NULL DEFAULT 0,
    total_paye NUMERIC NOT NULL DEFAULT 0,
    total_nssf NUMERIC NOT NULL DEFAULT 0,
    total_sha NUMERIC NOT NULL DEFAULT 0,
    total_housing_levy NUMERIC NOT NULL DEFAULT 0,
    total_helb NUMERIC NOT NULL DEFAULT 0,
    total_net NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    journal_entry_id UUID,
    created_by JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    approved_by JSONB,
    approved_at TIMESTAMPTZ
);

CREATE TABLE payslips (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    pay_run_id UUID NOT NULL REFERENCES pay_runs(id),
    employee_id UUID NOT NULL REFERENCES employees(id),
    deductions JSONB NOT NULL,
    custom_deductions JSONB NOT NULL DEFAULT '[]'::jsonb,
    custom_earnings JSONB NOT NULL DEFAULT '[]'::jsonb
);

-- ============================================================
-- EXCHANGE RATES
-- ============================================================
CREATE TABLE exchange_rates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    from_ccy CHAR(3) NOT NULL,
    to_ccy CHAR(3) NOT NULL,
    rate_date DATE NOT NULL,
    rate_type TEXT NOT NULL DEFAULT 'Spot',
    rate NUMERIC NOT NULL,
    source TEXT NOT NULL DEFAULT 'manual',
    UNIQUE(entity_id, from_ccy, to_ccy, rate_date, rate_type)
);

-- ============================================================
-- FIXED ASSETS
-- ============================================================
CREATE TABLE fixed_assets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    asset_number TEXT NOT NULL,
    description TEXT NOT NULL,
    category TEXT NOT NULL,
    acquisition_date DATE NOT NULL,
    cost NUMERIC NOT NULL,
    residual_value NUMERIC NOT NULL DEFAULT 0,
    useful_life_months INTEGER NOT NULL,
    depreciation_method JSONB NOT NULL,
    accumulated_depreciation NUMERIC NOT NULL DEFAULT 0,
    net_book_value NUMERIC NOT NULL,
    gl_asset_account TEXT NOT NULL,
    gl_accum_depr_account TEXT NOT NULL,
    gl_depr_expense TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    disposal_date DATE,
    disposal_proceeds NUMERIC,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, asset_number)
);

-- ============================================================
-- INVENTORY
-- ============================================================
CREATE TABLE inventory_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    product_id UUID REFERENCES products(id),
    sku TEXT NOT NULL,
    description TEXT NOT NULL,
    uom TEXT NOT NULL DEFAULT 'Each',
    costing_method TEXT NOT NULL DEFAULT 'WeightedAvgCost',
    gl_inventory TEXT NOT NULL DEFAULT '1500',
    gl_cogs TEXT NOT NULL DEFAULT '6000',
    on_hand NUMERIC NOT NULL DEFAULT 0,
    committed NUMERIC NOT NULL DEFAULT 0,
    available NUMERIC NOT NULL DEFAULT 0,
    unit_cost NUMERIC NOT NULL DEFAULT 0,
    total_value NUMERIC NOT NULL DEFAULT 0,
    reorder_point NUMERIC,
    reorder_quantity NUMERIC,
    warehouse_id UUID,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, sku)
);

CREATE TABLE stock_movements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    item_id UUID NOT NULL REFERENCES inventory_items(id),
    movement_type TEXT NOT NULL,
    date DATE NOT NULL,
    quantity NUMERIC NOT NULL,
    unit_cost NUMERIC NOT NULL,
    total_cost NUMERIC NOT NULL,
    reference_type TEXT,
    reference_id UUID,
    warehouse_id UUID,
    notes TEXT,
    created_by JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- INVOICE TEMPLATES
-- ============================================================
CREATE TABLE invoice_templates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    name TEXT NOT NULL,
    logo_url TEXT,
    primary_color TEXT NOT NULL DEFAULT '#1a56db',
    secondary_color TEXT,
    font TEXT NOT NULL DEFAULT 'Inter',
    footer_text TEXT,
    show_bank_details BOOLEAN NOT NULL DEFAULT true,
    show_mpesa_paybill BOOLEAN NOT NULL DEFAULT true,
    layout TEXT NOT NULL DEFAULT 'Modern',
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- USERS & RBAC
-- ============================================================
CREATE TABLE era_users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    email TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'Viewer',
    is_active BOOLEAN NOT NULL DEFAULT true,
    invited_by UUID,
    last_login TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, email)
);

-- ============================================================
-- AUDIT EVENTS
-- ============================================================
CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    object_type TEXT NOT NULL,
    object_id UUID NOT NULL,
    actor JSONB NOT NULL,
    before_state JSONB,
    after_state JSONB,
    metadata JSONB,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_entity ON audit_events(entity_id);
CREATE INDEX idx_audit_object ON audit_events(entity_id, object_type, object_id);
CREATE INDEX idx_audit_timestamp ON audit_events(entity_id, timestamp DESC);

-- ============================================================
-- ATTACHMENTS
-- ============================================================
CREATE TABLE attachments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    linked_type TEXT NOT NULL,
    linked_id UUID NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    storage_key TEXT NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    uploaded_by JSONB NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attachments_linked ON attachments(linked_type, linked_id);

-- ============================================================
-- NOTIFICATIONS
-- ============================================================
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    channel TEXT NOT NULL,
    recipient TEXT NOT NULL,
    subject TEXT,
    body TEXT NOT NULL,
    related_type TEXT,
    related_id UUID,
    status TEXT NOT NULL DEFAULT 'queued',
    scheduled_at TIMESTAMPTZ,
    sent_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    read_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- SUPPLIER CREDIT NOTES
-- ============================================================
CREATE TABLE supplier_credit_notes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    vendor_id UUID NOT NULL REFERENCES vendors(id),
    credit_note_number TEXT NOT NULL,
    credit_note_date DATE NOT NULL,
    applies_to_bill UUID REFERENCES bills(id),
    gross_total NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    journal_entry_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- M-PESA TRANSACTION RECORDS
-- ============================================================
CREATE TABLE mpesa_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    receipt_number TEXT NOT NULL,
    transaction_type TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    phone_number TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    invoice_id UUID,
    payment_id UUID,
    reconciled BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mpesa_receipt ON mpesa_transactions(receipt_number);

-- ============================================================
-- RECEIPT CAPTURES (OCR)
-- ============================================================
CREATE TABLE receipt_captures (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    image_url TEXT NOT NULL,
    ocr_result JSONB,
    suggested_vendor_id UUID,
    suggested_bill_id UUID,
    status TEXT NOT NULL DEFAULT 'pending',
    captured_by JSONB NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ
);
