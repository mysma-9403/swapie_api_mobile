use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
}

impl fmt::Display for MessageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageStatus::Pending => write!(f, "pending"),
            MessageStatus::Sent => write!(f, "sent"),
            MessageStatus::Delivered => write!(f, "delivered"),
            MessageStatus::Failed => write!(f, "failed"),
        }
    }
}

impl FromStr for MessageStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(MessageStatus::Pending),
            "sent" => Ok(MessageStatus::Sent),
            "delivered" => Ok(MessageStatus::Delivered),
            "failed" => Ok(MessageStatus::Failed),
            _ => Err(format!("Invalid MessageStatus: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: i64,
    pub trade_id: i64,
    pub sender_id: Option<i64>,
    pub content: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub message_type: Option<String>,
    pub status: MessageStatus,
    pub is_read: bool,
    pub is_system_message: bool,
    pub metadata: Option<String>,
    pub idempotency_key: Option<String>,
    pub push_sent: bool,
    pub push_sent_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
