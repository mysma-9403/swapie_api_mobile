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
pub enum LockerProvider {
    Inpost,
    Orlen,
}

impl fmt::Display for LockerProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockerProvider::Inpost => write!(f, "inpost"),
            LockerProvider::Orlen => write!(f, "orlen"),
        }
    }
}

impl FromStr for LockerProvider {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "inpost" => Ok(LockerProvider::Inpost),
            "orlen" => Ok(LockerProvider::Orlen),
            _ => Err(format!("Invalid LockerProvider: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Locker {
    pub id: i64,
    pub name: String,
    pub provider: LockerProvider,
    pub address: String,
    pub city: String,
    pub zip_code: String,
    pub latitude: Decimal,
    pub longitude: Decimal,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeliveryOption {
    pub id: i64,
    pub slug: String,
    pub name: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub delivery_type: Option<String>,
    pub provider: Option<String>,
    pub price: Decimal,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
