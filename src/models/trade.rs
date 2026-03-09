use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TradeStatus {
    Pending,
    Accepted,
    Rejected,
    Countered,
    Shipped,
    Delivered,
    Completed,
    Disputed,
    Cancelled,
    AwaitingShipment,
}

impl fmt::Display for TradeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradeStatus::Pending => write!(f, "pending"),
            TradeStatus::Accepted => write!(f, "accepted"),
            TradeStatus::Rejected => write!(f, "rejected"),
            TradeStatus::Countered => write!(f, "countered"),
            TradeStatus::Shipped => write!(f, "shipped"),
            TradeStatus::Delivered => write!(f, "delivered"),
            TradeStatus::Completed => write!(f, "completed"),
            TradeStatus::Disputed => write!(f, "disputed"),
            TradeStatus::Cancelled => write!(f, "cancelled"),
            TradeStatus::AwaitingShipment => write!(f, "awaiting_shipment"),
        }
    }
}

impl FromStr for TradeStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TradeStatus::Pending),
            "accepted" => Ok(TradeStatus::Accepted),
            "rejected" => Ok(TradeStatus::Rejected),
            "countered" => Ok(TradeStatus::Countered),
            "shipped" => Ok(TradeStatus::Shipped),
            "delivered" => Ok(TradeStatus::Delivered),
            "completed" => Ok(TradeStatus::Completed),
            "disputed" => Ok(TradeStatus::Disputed),
            "cancelled" => Ok(TradeStatus::Cancelled),
            "awaiting_shipment" => Ok(TradeStatus::AwaitingShipment),
            _ => Err(format!("Invalid TradeStatus: {}", s)),
        }
    }
}

// ── Structs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Trade {
    pub id: i64,
    pub initiator_id: i64,
    pub recipient_id: i64,
    pub status: TradeStatus,
    pub cash_top_up: Option<Decimal>,
    pub top_up_payer: Option<String>,
    pub protection_fee: Option<Decimal>,
    pub shipping_cost: Option<Decimal>,
    pub initiator_shipping_cost: Option<Decimal>,
    pub recipient_shipping_cost: Option<Decimal>,
    pub initiator_delivery_method: Option<String>,
    pub initiator_locker: Option<String>,
    pub recipient_delivery_method: Option<String>,
    pub recipient_locker: Option<String>,
    pub initiator_paid: bool,
    pub recipient_paid: bool,
    pub initiator_paid_amount: Option<Decimal>,
    pub recipient_paid_amount: Option<Decimal>,
    pub initiator_confirmed_delivery: bool,
    pub recipient_confirmed_delivery: bool,
    pub initiator_confirmed_at: Option<NaiveDateTime>,
    pub recipient_confirmed_at: Option<NaiveDateTime>,
    pub has_dispute: bool,
    pub dispute_opened_by: Option<i64>,
    pub dispute_reason: Option<String>,
    pub dispute_opened_at: Option<NaiveDateTime>,
    pub dispute_resolved_at: Option<NaiveDateTime>,
    pub dispute_resolution: Option<String>,
    pub escrow_status: Option<String>,
    pub escrow_payment_source: Option<String>,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_transfer_id: Option<String>,
    pub escrow_released_at: Option<NaiveDateTime>,
    pub initiator_to_recipient_shipment_id: Option<String>,
    pub initiator_to_recipient_label_url: Option<String>,
    pub recipient_to_initiator_shipment_id: Option<String>,
    pub recipient_to_initiator_label_url: Option<String>,
    pub auto_complete_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TradeItem {
    pub id: i64,
    pub trade_id: i64,
    pub book_id: i64,
    pub owner_id: i64,
    pub book_snapshot: Option<String>,
}
