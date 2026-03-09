use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Media {
    pub id: i64,
    pub model_type: Option<String>,
    pub model_id: Option<i64>,
    pub collection_name: String,
    pub name: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub disk: String,
    pub size: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
