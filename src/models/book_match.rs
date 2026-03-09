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
pub enum MatchType {
    Exchange,
    Purchase,
}

impl fmt::Display for MatchType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchType::Exchange => write!(f, "exchange"),
            MatchType::Purchase => write!(f, "purchase"),
        }
    }
}

impl FromStr for MatchType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exchange" => Ok(MatchType::Exchange),
            "purchase" => Ok(MatchType::Purchase),
            _ => Err(format!("Invalid MatchType: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Pending,
    Accepted,
    Rejected,
    Completed,
}

impl fmt::Display for MatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchStatus::Pending => write!(f, "pending"),
            MatchStatus::Accepted => write!(f, "accepted"),
            MatchStatus::Rejected => write!(f, "rejected"),
            MatchStatus::Completed => write!(f, "completed"),
        }
    }
}

impl FromStr for MatchStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(MatchStatus::Pending),
            "accepted" => Ok(MatchStatus::Accepted),
            "rejected" => Ok(MatchStatus::Rejected),
            "completed" => Ok(MatchStatus::Completed),
            _ => Err(format!("Invalid MatchStatus: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookMatch {
    pub id: i64,
    pub book_owner_id: i64,
    pub interested_user_id: i64,
    pub owner_book_id: i64,
    pub interested_book_id: Option<i64>,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub match_type: MatchType,
    pub status: MatchStatus,
    pub matched_at: Option<NaiveDateTime>,
    pub accepted_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
