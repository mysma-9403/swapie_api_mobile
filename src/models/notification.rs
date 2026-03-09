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
pub enum DeviceType {
    Ios,
    Android,
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceType::Ios => write!(f, "ios"),
            DeviceType::Android => write!(f, "android"),
        }
    }
}

impl FromStr for DeviceType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ios" => Ok(DeviceType::Ios),
            "android" => Ok(DeviceType::Android),
            _ => Err(format!("Invalid DeviceType: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Notification {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub body: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub notification_type: Option<String>,
    pub data: Option<String>,
    pub is_read: bool,
    pub read_at: Option<NaiveDateTime>,
    pub push_sent: bool,
    pub push_sent_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceToken {
    pub id: i64,
    pub user_id: i64,
    pub fcm_token: String,
    pub device_type: DeviceType,
    pub is_active: bool,
    pub last_used_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
