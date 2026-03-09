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
pub enum SwipeType {
    Like,
    Superlike,
    Reject,
}

impl fmt::Display for SwipeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SwipeType::Like => write!(f, "like"),
            SwipeType::Superlike => write!(f, "superlike"),
            SwipeType::Reject => write!(f, "reject"),
        }
    }
}

impl FromStr for SwipeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "like" => Ok(SwipeType::Like),
            "superlike" => Ok(SwipeType::Superlike),
            "reject" => Ok(SwipeType::Reject),
            _ => Err(format!("Invalid SwipeType: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Swipe {
    pub id: i64,
    pub user_id: i64,
    pub book_id: i64,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub swipe_type: SwipeType,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Legacy swipe model using 'action' column instead of 'type'.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookSwipe {
    pub id: i64,
    pub user_id: i64,
    pub book_id: i64,
    pub action: SwipeType,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
