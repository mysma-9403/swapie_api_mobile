use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAddress {
    pub id: i64,
    pub user_id: i64,
    pub street: String,
    pub building_number: String,
    pub flat_number: Option<String>,
    pub zip_code: String,
    pub city: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserBlock {
    pub id: i64,
    pub blocker_id: i64,
    pub blocked_id: i64,
    pub reason: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
