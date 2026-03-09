use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BookType {
    Book,
    BoardGame,
}

impl fmt::Display for BookType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookType::Book => write!(f, "book"),
            BookType::BoardGame => write!(f, "board_game"),
        }
    }
}

impl FromStr for BookType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "book" => Ok(BookType::Book),
            "board_game" => Ok(BookType::BoardGame),
            _ => Err(format!("Invalid BookType: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BookCondition {
    New,
    UsedVeryGood,
    UsedGood,
    UsedAcceptable,
}

impl fmt::Display for BookCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookCondition::New => write!(f, "new"),
            BookCondition::UsedVeryGood => write!(f, "used_very_good"),
            BookCondition::UsedGood => write!(f, "used_good"),
            BookCondition::UsedAcceptable => write!(f, "used_acceptable"),
        }
    }
}

impl FromStr for BookCondition {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "new" => Ok(BookCondition::New),
            "used_very_good" => Ok(BookCondition::UsedVeryGood),
            "used_good" => Ok(BookCondition::UsedGood),
            "used_acceptable" => Ok(BookCondition::UsedAcceptable),
            _ => Err(format!("Invalid BookCondition: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BookStatus {
    Active,
    Inactive,
    PendingExchange,
    Sold,
    Matched,
}

impl fmt::Display for BookStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookStatus::Active => write!(f, "active"),
            BookStatus::Inactive => write!(f, "inactive"),
            BookStatus::PendingExchange => write!(f, "pending_exchange"),
            BookStatus::Sold => write!(f, "sold"),
            BookStatus::Matched => write!(f, "matched"),
        }
    }
}

impl FromStr for BookStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(BookStatus::Active),
            "inactive" => Ok(BookStatus::Inactive),
            "pending_exchange" => Ok(BookStatus::PendingExchange),
            "sold" => Ok(BookStatus::Sold),
            "matched" => Ok(BookStatus::Matched),
            _ => Err(format!("Invalid BookStatus: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BookFormat {
    Pocket,
    Standard,
    Large,
}

impl fmt::Display for BookFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookFormat::Pocket => write!(f, "pocket"),
            BookFormat::Standard => write!(f, "standard"),
            BookFormat::Large => write!(f, "large"),
        }
    }
}

impl FromStr for BookFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pocket" => Ok(BookFormat::Pocket),
            "standard" => Ok(BookFormat::Standard),
            "large" => Ok(BookFormat::Large),
            _ => Err(format!("Invalid BookFormat: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Book {
    pub id: i64,
    pub user_id: i64,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub book_type: BookType,
    pub title: String,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub description: Option<String>,
    pub condition: BookCondition,
    pub status: BookStatus,
    pub for_exchange: bool,
    pub for_sale: bool,
    pub price: Option<Decimal>,
    pub location: Option<String>,
    pub latitude: Option<Decimal>,
    pub longitude: Option<Decimal>,
    pub category: Option<String>,
    pub language: Option<String>,
    pub pages_count: Option<i32>,
    pub book_format: Option<BookFormat>,
    pub views_count: i32,
    pub likes_count: i32,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
    pub playing_time: Option<i32>,
    pub age_rating: Option<i32>,
    pub wanted_isbn: Option<String>,
    pub wanted_title: Option<String>,
    pub use_profile_filters: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookImage {
    pub id: i64,
    pub book_id: i64,
    pub image_path: String,
    pub is_primary: bool,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookChange {
    pub id: i64,
    pub book_id: i64,
    pub user_id: i64,
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: NaiveDateTime,
}
