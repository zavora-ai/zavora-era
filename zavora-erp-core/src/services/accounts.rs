use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::account::*;
use crate::ledger::coa_template::{kenya_standard_coa, CoaTemplate};
use crate::types::AgentOrUserId;

/// Create a new account in the chart of accounts.
pub async fn create_account(
    engine: &ErpEngine,
    req: CreateAccountRequest,
    created_by: &AgentOrUserId,
) -> ErpResult<Account> {
    // Validate code doesn't already exist
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE entity_id = $1 AND code = $2)",
    )
    .bind(engine.entity_id())
    .bind(&req.code)
    .fetch_one(engine.pool())
    .await?;

    if exists {
        return Err(ErpError::Duplicate {
            message: format!("Account code {} already exists", req.code),
        });
    }

    // Validate parent exists if specified
    if let Some(ref parent) = req.parent_code {
        let parent_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE entity_id = $1 AND code = $2)",
        )
        .bind(engine.entity_id())
        .bind(parent)
        .fetch_one(engine.pool())
        .await?;

        if !parent_exists {
            return Err(ErpError::AccountNotFound {
                code: parent.clone(),
            });
        }
    }

    let id = Uuid::new_v4();
    let now = Utc::now();
    let account_type_str = serde_json::to_value(&req.account_type)
        .unwrap_or_default()
        .as_str()
        .unwrap_or("asset")
        .to_string();

    sqlx::query(
        r#"INSERT INTO accounts 
           (id, entity_id, code, name, account_type, parent_code, currency, is_control, is_active, tags, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
    .bind(&req.code)
    .bind(&req.name)
    .bind(&account_type_str)
    .bind(&req.parent_code)
    .bind(&req.currency)
    .bind(req.is_control)
    .bind(true)
    .bind(serde_json::to_value(&req.tags).unwrap_or_default())
    .bind(now)
    .execute(engine.pool())
    .await?;

    Ok(Account {
        id,
        entity_id: engine.entity_id(),
        code: req.code,
        name: req.name,
        account_type: account_type_str,
        parent_code: req.parent_code,
        currency: req.currency,
        is_control: req.is_control,
        is_active: true,
        tags: serde_json::to_value(&req.tags).unwrap_or_default(),
        created_at: now,
    })
}

/// Seed the chart of accounts from a template.
pub async fn seed_coa(
    engine: &ErpEngine,
    template: &CoaTemplate,
    created_by: &AgentOrUserId,
) -> ErpResult<u32> {
    let accounts = match template {
        CoaTemplate::KenyaStandard => kenya_standard_coa(),
        CoaTemplate::Minimal => kenya_standard_coa()
            .into_iter()
            .filter(|a| a.parent_code.is_none())
            .collect(),
        CoaTemplate::Custom => return Ok(0),
    };

    let mut count = 0u32;
    for req in accounts {
        match create_account(engine, req, created_by).await {
            Ok(_) => count += 1,
            Err(ErpError::Duplicate { .. }) => {
                // Already exists, skip
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(count)
}

/// Get account by code.
pub async fn get_account(engine: &ErpEngine, code: &str) -> ErpResult<Account> {
    sqlx::query_as::<_, Account>(
        "SELECT * FROM accounts WHERE entity_id = $1 AND code = $2",
    )
    .bind(engine.entity_id())
    .bind(code)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::AccountNotFound {
        code: code.to_string(),
    })
}

/// List all accounts for the entity.
pub async fn list_accounts(engine: &ErpEngine, active_only: bool) -> ErpResult<Vec<Account>> {
    let accounts = if active_only {
        sqlx::query_as::<_, Account>(
            "SELECT * FROM accounts WHERE entity_id = $1 AND is_active = true ORDER BY code",
        )
        .bind(engine.entity_id())
        .fetch_all(engine.pool())
        .await?
    } else {
        sqlx::query_as::<_, Account>(
            "SELECT * FROM accounts WHERE entity_id = $1 ORDER BY code",
        )
        .bind(engine.entity_id())
        .fetch_all(engine.pool())
        .await?
    };

    Ok(accounts)
}

/// Update an account.
pub async fn update_account(
    engine: &ErpEngine,
    code: &str,
    req: UpdateAccountRequest,
) -> ErpResult<Account> {
    // Verify account exists
    let mut account = get_account(engine, code).await?;

    if let Some(name) = req.name {
        account.name = name;
    }
    if let Some(parent) = req.parent_code {
        account.parent_code = parent;
    }
    if let Some(currency) = req.currency {
        account.currency = currency;
    }
    if let Some(is_control) = req.is_control {
        account.is_control = is_control;
    }
    if let Some(is_active) = req.is_active {
        account.is_active = is_active;
    }
    if let Some(tags) = req.tags {
        account.tags = serde_json::to_value(&tags).unwrap_or_default();
    }

    sqlx::query(
        r#"UPDATE accounts 
           SET name = $1, parent_code = $2, currency = $3, is_control = $4, is_active = $5, tags = $6
           WHERE entity_id = $7 AND code = $8"#,
    )
    .bind(&account.name)
    .bind(&account.parent_code)
    .bind(&account.currency)
    .bind(account.is_control)
    .bind(account.is_active)
    .bind(&account.tags)
    .bind(engine.entity_id())
    .bind(code)
    .execute(engine.pool())
    .await?;

    Ok(account)
}
