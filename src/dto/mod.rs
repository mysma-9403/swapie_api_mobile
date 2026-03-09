use serde::{Deserialize, Serialize};

// ── Generic API response wrapper ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Build a successful response carrying data and a human-readable message.
    pub fn success(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }
}

impl ApiResponse<()> {
    /// Build an error response (no data payload).
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }

    /// Build a success response that carries only a message (no data payload).
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }
}

// ── Paginated response ───────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub success: bool,
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub current_page: u32,
    pub per_page: u32,
    pub total: u64,
    pub last_page: u32,
}

impl PaginationMeta {
    pub fn new(current_page: u32, per_page: u32, total: u64) -> Self {
        let last_page = if total == 0 {
            1
        } else {
            ((total as f64) / (per_page as f64)).ceil() as u32
        };
        Self {
            current_page,
            per_page,
            total,
            last_page,
        }
    }
}

impl<T: Serialize> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, current_page: u32, per_page: u32, total: u64) -> Self {
        Self {
            success: true,
            data,
            meta: PaginationMeta::new(current_page, per_page, total),
        }
    }
}

// ── Pagination query params ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PaginationParams {
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }

    pub fn offset(&self) -> u64 {
        ((self.page() - 1) as u64) * (self.per_page() as u64)
    }
}
