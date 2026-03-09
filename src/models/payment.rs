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
pub enum PaymentStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Expired,
}

impl fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentStatus::Pending => write!(f, "pending"),
            PaymentStatus::Processing => write!(f, "processing"),
            PaymentStatus::Completed => write!(f, "completed"),
            PaymentStatus::Failed => write!(f, "failed"),
            PaymentStatus::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for PaymentStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(PaymentStatus::Pending),
            "processing" => Ok(PaymentStatus::Processing),
            "completed" => Ok(PaymentStatus::Completed),
            "failed" => Ok(PaymentStatus::Failed),
            "expired" => Ok(PaymentStatus::Expired),
            _ => Err(format!("Invalid PaymentStatus: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaymentRequest {
    pub id: i64,
    pub user_id: i64,
    pub external_order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_payment_method_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub amount: Decimal,
    pub method: Option<String>,
    pub status: PaymentStatus,
    pub error_message: Option<String>,
    pub processed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StripePaymentMethod {
    pub id: i64,
    pub user_id: i64,
    pub stripe_payment_method_id: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub method_type: String,
    pub card_brand: Option<String>,
    pub card_last_four: Option<String>,
    pub card_exp_month: Option<i32>,
    pub card_exp_year: Option<i32>,
    pub is_default: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
