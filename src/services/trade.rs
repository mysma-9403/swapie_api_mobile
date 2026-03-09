use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::config::SharedConfig;
use crate::errors::ApiError;
use crate::models::{Book, Trade, TradeItem, TradeStatus};

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateOfferData {
    pub recipient_id: i64,
    pub initiator_book_ids: Vec<i64>,
    pub recipient_book_ids: Vec<i64>,
    pub cash_top_up: Option<Decimal>,
    pub top_up_payer: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CounterOfferData {
    pub initiator_book_ids: Vec<i64>,
    pub recipient_book_ids: Vec<i64>,
    pub cash_top_up: Option<Decimal>,
    pub top_up_payer: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FinalizeTradeData {
    pub initiator_delivery_method: Option<String>,
    pub initiator_locker: Option<String>,
    pub recipient_delivery_method: Option<String>,
    pub recipient_locker: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TradeDetail {
    #[serde(flatten)]
    pub trade: Trade,
    pub items: Vec<TradeItem>,
}

#[derive(Debug, Serialize)]
pub struct CostPreview {
    pub initiator_shipping_cost: Decimal,
    pub recipient_shipping_cost: Decimal,
    pub protection_fee: Decimal,
    pub total_initiator: Decimal,
    pub total_recipient: Decimal,
}

#[derive(Debug, Serialize)]
pub struct DeliveryStatusInfo {
    pub trade_id: i64,
    pub initiator_to_recipient_shipment_id: Option<String>,
    pub initiator_to_recipient_label_url: Option<String>,
    pub recipient_to_initiator_shipment_id: Option<String>,
    pub recipient_to_initiator_label_url: Option<String>,
    pub initiator_confirmed_delivery: bool,
    pub recipient_confirmed_delivery: bool,
    pub status: String,
}

// ── State machine ───────────────────────────────────────────────────────────

/// Validate that a trade status transition is allowed.
fn validate_status_transition(
    current: &TradeStatus,
    target: &TradeStatus,
) -> Result<(), ApiError> {
    let allowed = match current {
        TradeStatus::Pending => vec![
            TradeStatus::Accepted,
            TradeStatus::Rejected,
            TradeStatus::Countered,
            TradeStatus::Cancelled,
        ],
        TradeStatus::Countered => vec![
            TradeStatus::Accepted,
            TradeStatus::Rejected,
            TradeStatus::Countered,
            TradeStatus::Cancelled,
        ],
        TradeStatus::Accepted => vec![
            TradeStatus::AwaitingShipment,
            TradeStatus::Cancelled,
            TradeStatus::Disputed,
        ],
        TradeStatus::AwaitingShipment => vec![
            TradeStatus::Shipped,
            TradeStatus::Cancelled,
            TradeStatus::Disputed,
        ],
        TradeStatus::Shipped => vec![
            TradeStatus::Delivered,
            TradeStatus::Disputed,
        ],
        TradeStatus::Delivered => vec![
            TradeStatus::Completed,
            TradeStatus::Disputed,
        ],
        TradeStatus::Disputed => vec![
            TradeStatus::Completed,
            TradeStatus::Cancelled,
        ],
        TradeStatus::Completed | TradeStatus::Rejected | TradeStatus::Cancelled => vec![],
    };

    if allowed.contains(target) {
        Ok(())
    } else {
        Err(ApiError::bad_request("trade.invalid_status_transition"))
    }
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct TradeService;

impl TradeService {
    /// Create a new trade offer, snapshotting books at the time of creation.
    pub async fn create_offer(
        pool: &MySqlPool,
        initiator_id: i64,
        data: CreateOfferData,
    ) -> Result<TradeDetail, ApiError> {
        if initiator_id == data.recipient_id {
            return Err(ApiError::bad_request("trade.cannot_trade_with_self"));
        }

        // Verify recipient exists
        let recipient_exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE id = ?",
        )
        .bind(data.recipient_id)
        .fetch_one(pool)
        .await?;
        if recipient_exists.0 == 0 {
            return Err(ApiError::not_found("user.not_found"));
        }

        let mut tx = pool.begin().await?;
        let now = Utc::now().naive_utc();

        // Create the trade record
        let result = sqlx::query(
            r#"
            INSERT INTO trades (
                initiator_id, recipient_id, status,
                cash_top_up, top_up_payer,
                initiator_paid, recipient_paid,
                initiator_confirmed_delivery, recipient_confirmed_delivery,
                has_dispute,
                created_at, updated_at
            ) VALUES (?, ?, 'pending', ?, ?, 0, 0, 0, 0, 0, ?, ?)
            "#,
        )
        .bind(initiator_id)
        .bind(data.recipient_id)
        .bind(&data.cash_top_up)
        .bind(&data.top_up_payer)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let trade_id = result.last_insert_id() as i64;

        // Insert initiator's book items with snapshots
        for book_id in &data.initiator_book_ids {
            let book = sqlx::query_as::<_, Book>(
                "SELECT * FROM books WHERE id = ? AND user_id = ? AND deleted_at IS NULL AND status = 'active'",
            )
            .bind(book_id)
            .bind(initiator_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::bad_request("trade.book_not_available"))?;

            let snapshot = serde_json::to_string(&book)?;

            sqlx::query(
                "INSERT INTO trade_items (trade_id, book_id, owner_id, book_snapshot) VALUES (?, ?, ?, ?)",
            )
            .bind(trade_id)
            .bind(book_id)
            .bind(initiator_id)
            .bind(&snapshot)
            .execute(&mut *tx)
            .await?;
        }

        // Insert recipient's book items with snapshots
        for book_id in &data.recipient_book_ids {
            let book = sqlx::query_as::<_, Book>(
                "SELECT * FROM books WHERE id = ? AND user_id = ? AND deleted_at IS NULL AND status = 'active'",
            )
            .bind(book_id)
            .bind(data.recipient_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::bad_request("trade.book_not_available"))?;

            let snapshot = serde_json::to_string(&book)?;

            sqlx::query(
                "INSERT INTO trade_items (trade_id, book_id, owner_id, book_snapshot) VALUES (?, ?, ?, ?)",
            )
            .bind(trade_id)
            .bind(book_id)
            .bind(data.recipient_id)
            .bind(&snapshot)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Self::get_offer(pool, trade_id, initiator_id).await
    }

    /// Accept a trade offer (step 1 -- marks as accepted).
    pub async fn accept_offer(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;

        // Only recipient can accept
        if trade.recipient_id != user_id {
            return Err(ApiError::forbidden());
        }

        validate_status_transition(&trade.status, &TradeStatus::Accepted)?;

        sqlx::query(
            "UPDATE trades SET status = 'accepted', updated_at = NOW() WHERE id = ?",
        )
        .bind(trade_id)
        .execute(pool)
        .await?;

        // Mark books as pending_exchange
        sqlx::query(
            r#"
            UPDATE books SET status = 'pending_exchange', updated_at = NOW()
            WHERE id IN (SELECT book_id FROM trade_items WHERE trade_id = ?)
              AND deleted_at IS NULL
            "#,
        )
        .bind(trade_id)
        .execute(pool)
        .await?;

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Finalize a trade (step 2) -- set delivery details and generate shipment labels.
    pub async fn finalize_trade(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
        data: FinalizeTradeData,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;
        validate_status_transition(&trade.status, &TradeStatus::AwaitingShipment)?;

        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE trades SET
                status = 'awaiting_shipment',
                initiator_delivery_method = COALESCE(?, initiator_delivery_method),
                initiator_locker = COALESCE(?, initiator_locker),
                recipient_delivery_method = COALESCE(?, recipient_delivery_method),
                recipient_locker = COALESCE(?, recipient_locker),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(&data.initiator_delivery_method)
        .bind(&data.initiator_locker)
        .bind(&data.recipient_delivery_method)
        .bind(&data.recipient_locker)
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Reject a trade offer.
    pub async fn reject_offer(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;

        // Only recipient can reject
        if trade.recipient_id != user_id {
            return Err(ApiError::forbidden());
        }

        validate_status_transition(&trade.status, &TradeStatus::Rejected)?;

        let mut tx = pool.begin().await?;

        sqlx::query(
            "UPDATE trades SET status = 'rejected', updated_at = NOW() WHERE id = ?",
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        // Release books back to active status
        sqlx::query(
            r#"
            UPDATE books SET status = 'active', updated_at = NOW()
            WHERE id IN (SELECT book_id FROM trade_items WHERE trade_id = ?)
              AND status = 'pending_exchange'
              AND deleted_at IS NULL
            "#,
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Cancel a trade (only the initiator can cancel).
    pub async fn cancel_offer(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;

        if trade.initiator_id != user_id {
            return Err(ApiError::forbidden());
        }

        validate_status_transition(&trade.status, &TradeStatus::Cancelled)?;

        let mut tx = pool.begin().await?;

        sqlx::query(
            "UPDATE trades SET status = 'cancelled', updated_at = NOW() WHERE id = ?",
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        // Release books back to active
        sqlx::query(
            r#"
            UPDATE books SET status = 'active', updated_at = NOW()
            WHERE id IN (SELECT book_id FROM trade_items WHERE trade_id = ?)
              AND status = 'pending_exchange'
              AND deleted_at IS NULL
            "#,
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Create a counter offer by updating the existing trade with new items.
    pub async fn counter_offer(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
        data: CounterOfferData,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;
        validate_status_transition(&trade.status, &TradeStatus::Countered)?;

        let mut tx = pool.begin().await?;

        // Update trade status and top-up
        sqlx::query(
            r#"
            UPDATE trades SET
                status = 'countered',
                cash_top_up = ?,
                top_up_payer = ?,
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(&data.cash_top_up)
        .bind(&data.top_up_payer)
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        // Remove old trade items and release those books
        sqlx::query(
            r#"
            UPDATE books SET status = 'active', updated_at = NOW()
            WHERE id IN (SELECT book_id FROM trade_items WHERE trade_id = ?)
              AND status = 'pending_exchange'
              AND deleted_at IS NULL
            "#,
        )
        .bind(trade_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM trade_items WHERE trade_id = ?")
            .bind(trade_id)
            .execute(&mut *tx)
            .await?;

        // Insert new initiator items
        for book_id in &data.initiator_book_ids {
            let book = sqlx::query_as::<_, Book>(
                "SELECT * FROM books WHERE id = ? AND user_id = ? AND deleted_at IS NULL AND status = 'active'",
            )
            .bind(book_id)
            .bind(trade.initiator_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::bad_request("trade.book_not_available"))?;

            let snapshot = serde_json::to_string(&book)?;

            sqlx::query(
                "INSERT INTO trade_items (trade_id, book_id, owner_id, book_snapshot) VALUES (?, ?, ?, ?)",
            )
            .bind(trade_id)
            .bind(book_id)
            .bind(trade.initiator_id)
            .bind(&snapshot)
            .execute(&mut *tx)
            .await?;
        }

        // Insert new recipient items
        for book_id in &data.recipient_book_ids {
            let book = sqlx::query_as::<_, Book>(
                "SELECT * FROM books WHERE id = ? AND user_id = ? AND deleted_at IS NULL AND status = 'active'",
            )
            .bind(book_id)
            .bind(trade.recipient_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::bad_request("trade.book_not_available"))?;

            let snapshot = serde_json::to_string(&book)?;

            sqlx::query(
                "INSERT INTO trade_items (trade_id, book_id, owner_id, book_snapshot) VALUES (?, ?, ?, ?)",
            )
            .bind(trade_id)
            .bind(book_id)
            .bind(trade.recipient_id)
            .bind(&snapshot)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Get full trade details with items.
    pub async fn get_offer(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;

        let items = sqlx::query_as::<_, TradeItem>(
            "SELECT * FROM trade_items WHERE trade_id = ?",
        )
        .bind(trade_id)
        .fetch_all(pool)
        .await?;

        Ok(TradeDetail { trade, items })
    }

    /// Calculate shipping cost preview for a trade.
    pub async fn get_cost_preview(
        pool: &MySqlPool,
        trade_id: i64,
        delivery_method: &str,
    ) -> Result<CostPreview, ApiError> {
        let _trade = Self::fetch_trade(pool, trade_id).await?;

        // Fetch fee config from settings table
        let protection_fee_row: Option<(String,)> = sqlx::query_as(
            "SELECT `value` FROM settings WHERE `key` = 'protection_fee' LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;
        let protection_fee: Decimal = protection_fee_row
            .and_then(|r| r.0.parse().ok())
            .unwrap_or_else(|| Decimal::new(299, 2)); // default 2.99

        let shipping_cost_row: Option<(String,)> = sqlx::query_as(
            "SELECT `value` FROM settings WHERE `key` = ? LIMIT 1",
        )
        .bind(format!("shipping_cost_{}", delivery_method))
        .fetch_optional(pool)
        .await?;
        let per_party_shipping: Decimal = shipping_cost_row
            .and_then(|r| r.0.parse().ok())
            .unwrap_or_else(|| Decimal::new(999, 2)); // default 9.99

        Ok(CostPreview {
            initiator_shipping_cost: per_party_shipping,
            recipient_shipping_cost: per_party_shipping,
            protection_fee,
            total_initiator: per_party_shipping + protection_fee,
            total_recipient: per_party_shipping,
        })
    }

    /// Confirm delivery for one party in the trade.
    pub async fn confirm_delivery(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;

        let now = Utc::now().naive_utc();

        if trade.initiator_id == user_id {
            sqlx::query(
                "UPDATE trades SET initiator_confirmed_delivery = 1, initiator_confirmed_at = ?, updated_at = NOW() WHERE id = ?",
            )
            .bind(now)
            .bind(trade_id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE trades SET recipient_confirmed_delivery = 1, recipient_confirmed_at = ?, updated_at = NOW() WHERE id = ?",
            )
            .bind(now)
            .bind(trade_id)
            .execute(pool)
            .await?;
        }

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Check if both parties confirmed delivery; if so, complete the trade.
    pub async fn check_trade_completion(
        pool: &MySqlPool,
        trade_id: i64,
    ) -> Result<bool, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;

        if trade.initiator_confirmed_delivery && trade.recipient_confirmed_delivery {
            let mut tx = pool.begin().await?;

            sqlx::query(
                "UPDATE trades SET status = 'completed', updated_at = NOW() WHERE id = ?",
            )
            .bind(trade_id)
            .execute(&mut *tx)
            .await?;

            // Mark books as sold/exchanged
            sqlx::query(
                r#"
                UPDATE books SET status = 'sold', updated_at = NOW()
                WHERE id IN (SELECT book_id FROM trade_items WHERE trade_id = ?)
                  AND deleted_at IS NULL
                "#,
            )
            .bind(trade_id)
            .execute(&mut *tx)
            .await?;

            // Release escrow if applicable
            if trade.escrow_status.as_deref() == Some("held") {
                sqlx::query(
                    "UPDATE trades SET escrow_status = 'released', escrow_released_at = NOW(), updated_at = NOW() WHERE id = ?",
                )
                .bind(trade_id)
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Open a dispute on a trade.
    pub async fn open_dispute(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
        reason: &str,
    ) -> Result<TradeDetail, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;
        Self::verify_participant(&trade, user_id)?;
        validate_status_transition(&trade.status, &TradeStatus::Disputed)?;

        sqlx::query(
            r#"
            UPDATE trades SET
                status = 'disputed',
                has_dispute = 1,
                dispute_opened_by = ?,
                dispute_reason = ?,
                dispute_opened_at = NOW(),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(user_id)
        .bind(reason)
        .bind(trade_id)
        .execute(pool)
        .await?;

        Self::get_offer(pool, trade_id, user_id).await
    }

    /// Get delivery tracking status for a trade.
    pub async fn get_delivery_status(
        pool: &MySqlPool,
        trade_id: i64,
    ) -> Result<DeliveryStatusInfo, ApiError> {
        let trade = Self::fetch_trade(pool, trade_id).await?;

        Ok(DeliveryStatusInfo {
            trade_id: trade.id,
            initiator_to_recipient_shipment_id: trade.initiator_to_recipient_shipment_id,
            initiator_to_recipient_label_url: trade.initiator_to_recipient_label_url,
            recipient_to_initiator_shipment_id: trade.recipient_to_initiator_shipment_id,
            recipient_to_initiator_label_url: trade.recipient_to_initiator_label_url,
            initiator_confirmed_delivery: trade.initiator_confirmed_delivery,
            recipient_confirmed_delivery: trade.recipient_confirmed_delivery,
            status: trade.status.to_string(),
        })
    }

    /// Get books belonging to a user that are not currently involved in any active trade.
    pub async fn get_user_inventory(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<Book>, ApiError> {
        let books = sqlx::query_as::<_, Book>(
            r#"
            SELECT b.* FROM books b
            WHERE b.user_id = ?
              AND b.deleted_at IS NULL
              AND b.status = 'active'
              AND b.id NOT IN (
                  SELECT ti.book_id FROM trade_items ti
                  INNER JOIN trades t ON t.id = ti.trade_id
                  WHERE t.status IN ('pending', 'accepted', 'countered', 'awaiting_shipment', 'shipped')
              )
            ORDER BY b.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(books)
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    async fn fetch_trade(pool: &MySqlPool, trade_id: i64) -> Result<Trade, ApiError> {
        sqlx::query_as::<_, Trade>("SELECT * FROM trades WHERE id = ?")
            .bind(trade_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::not_found("trade.not_found"))
    }

    fn verify_participant(trade: &Trade, user_id: i64) -> Result<(), ApiError> {
        if trade.initiator_id != user_id && trade.recipient_id != user_id {
            return Err(ApiError::forbidden());
        }
        Ok(())
    }
}
