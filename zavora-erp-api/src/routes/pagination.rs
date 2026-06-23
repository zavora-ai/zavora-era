//! Shared pagination: a `limit`/`offset` query extractor and a response envelope
//! used by every list endpoint (Requirement 3).

use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 500;

/// Optional `?limit=&offset=` query parameters.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl PaginationParams {
    /// Default 50, clamped to [1, 500].
    pub fn effective_limit(&self) -> i64 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
    /// Default 0, never negative.
    pub fn effective_offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

/// `{ data, total_count, limit, offset, has_more }` list envelope.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total_count: i64, params: &PaginationParams) -> Self {
        let limit = params.effective_limit();
        let offset = params.effective_offset();
        Self {
            data,
            total_count,
            limit,
            offset,
            has_more: offset + limit < total_count,
        }
    }
}
