use thiserror::Error;
use uuid::Uuid;

/// Central error type for all ERP engine operations.
#[derive(Debug, Error)]
pub enum ErpError {
    // === Journal / Ledger errors ===
    #[error("journal entry is unbalanced: debits={debits}, credits={credits}")]
    Unbalanced {
        debits: rust_decimal::Decimal,
        credits: rust_decimal::Decimal,
    },

    #[error("period {period_id} is closed; cannot post entry dated {date}")]
    PeriodClosed {
        period_id: Uuid,
        date: chrono::NaiveDate,
    },

    #[error("period '{period_name}' is {status}; posting is not allowed")]
    PeriodClosedDetailed {
        period_name: String,
        status: String,
        period_id: Uuid,
    },

    #[error("account not found: {code}")]
    AccountNotFound { code: String },

    #[error("account is inactive: {code}")]
    AccountInactive { code: String },

    #[error("duplicate reference: {reference}")]
    DuplicateReference { reference: String },

    #[error("control account {code} cannot be posted directly")]
    ControlAccountViolation { code: String },

    #[error("FX rate not found for {from_ccy}/{to_ccy} on {date}")]
    FxRateNotFound {
        from_ccy: String,
        to_ccy: String,
        date: chrono::NaiveDate,
    },

    // === Validation errors ===
    #[error("validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: Uuid },

    #[error("duplicate entry: {message}")]
    Duplicate { message: String },

    // === Credit Limit ===
    #[error("credit limit exceeded for customer {customer_name}: outstanding={outstanding}, new_invoice={invoice_total}, limit={credit_limit}")]
    CreditLimitExceeded {
        customer_name: String,
        customer_id: Uuid,
        outstanding: rust_decimal::Decimal,
        invoice_total: rust_decimal::Decimal,
        credit_limit: rust_decimal::Decimal,
    },

    // === Inventory ===
    #[error("insufficient stock for item {sku}: available={available}, requested={requested}")]
    InsufficientStock {
        sku: String,
        available: rust_decimal::Decimal,
        requested: rust_decimal::Decimal,
    },

    // === Authorization ===
    #[error("permission denied: {action} requires role {required_role}")]
    PermissionDenied {
        action: String,
        required_role: String,
    },

    // === Payment ===
    #[error("payment processing error: {message}")]
    PaymentError { message: String },

    #[error("invoice {invoice_id} overpayment: balance={balance}, payment={amount}")]
    Overpayment {
        invoice_id: Uuid,
        balance: rust_decimal::Decimal,
        amount: rust_decimal::Decimal,
    },

    // === Infrastructure ===
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type alias for ERP operations.
pub type ErpResult<T> = Result<T, ErpError>;
