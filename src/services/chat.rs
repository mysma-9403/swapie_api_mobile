use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

use crate::dto::{PaginatedResponse, PaginationParams};
use crate::errors::ApiError;
use crate::models::Message;

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct StartConversationData {
    pub other_user_id: i64,
    pub initial_message: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct InboxEntry {
    pub trade_id: i64,
    pub other_user_id: i64,
    pub other_username: String,
    pub other_avatar_id: Option<i64>,
    pub last_message_content: Option<String>,
    pub last_message_at: Option<chrono::NaiveDateTime>,
    pub unread_count: i64,
    pub trade_status: String,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct ChatService;

impl ChatService {
    /// Start a conversation between two users. Creates a trade in 'pending'
    /// status if none exists yet, then returns the trade_id.
    pub async fn start_conversation(
        pool: &MySqlPool,
        user_id: i64,
        data: StartConversationData,
    ) -> Result<i64, ApiError> {
        if user_id == data.other_user_id {
            return Err(ApiError::bad_request("chat.cannot_message_self"));
        }

        // Check for an existing trade / conversation between the two users
        let existing: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM trades
            WHERE (
                (initiator_id = ? AND recipient_id = ?)
                OR (initiator_id = ? AND recipient_id = ?)
            )
              AND status NOT IN ('completed', 'cancelled', 'rejected')
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(data.other_user_id)
        .bind(data.other_user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        let trade_id = if let Some((id,)) = existing {
            id
        } else {
            // Create a new trade as a conversation container
            let now = Utc::now().naive_utc();
            let result = sqlx::query(
                r#"
                INSERT INTO trades (
                    initiator_id, recipient_id, status,
                    initiator_paid, recipient_paid,
                    initiator_confirmed_delivery, recipient_confirmed_delivery,
                    has_dispute, created_at, updated_at
                ) VALUES (?, ?, 'pending', 0, 0, 0, 0, 0, ?, ?)
                "#,
            )
            .bind(user_id)
            .bind(data.other_user_id)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;

            result.last_insert_id() as i64
        };

        // Send initial message if provided
        if let Some(content) = data.initial_message {
            Self::send_message(pool, trade_id, user_id, &content, None).await?;
        }

        Ok(trade_id)
    }

    /// Send a message in a trade conversation.
    ///
    /// Validates that the sender is a participant, checks the idempotency key
    /// to prevent duplicate messages, creates the message with 'sent' status,
    /// and triggers a push notification to the other party.
    pub async fn send_message(
        pool: &MySqlPool,
        trade_id: i64,
        sender_id: i64,
        content: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Message, ApiError> {
        // Validate sender is a participant
        let trade: (i64, i64) = sqlx::query_as(
            "SELECT initiator_id, recipient_id FROM trades WHERE id = ?",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("trade.not_found"))?;

        if trade.0 != sender_id && trade.1 != sender_id {
            return Err(ApiError::forbidden());
        }

        // Check idempotency key for duplicate prevention
        if let Some(key) = idempotency_key {
            let existing: Option<Message> = sqlx::query_as(
                "SELECT * FROM messages WHERE trade_id = ? AND idempotency_key = ? LIMIT 1",
            )
            .bind(trade_id)
            .bind(key)
            .fetch_optional(pool)
            .await?;

            if let Some(msg) = existing {
                return Ok(msg);
            }
        }

        let now = Utc::now().naive_utc();

        let result = sqlx::query(
            r#"
            INSERT INTO messages (
                trade_id, sender_id, content, `type`, status,
                is_read, is_system_message, idempotency_key,
                push_sent, created_at, updated_at
            ) VALUES (?, ?, ?, 'text', 'sent', 0, 0, ?, 0, ?, ?)
            "#,
        )
        .bind(trade_id)
        .bind(sender_id)
        .bind(content)
        .bind(idempotency_key)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let message_id = result.last_insert_id() as i64;

        // Determine the other party for push notification
        let other_user_id = if trade.0 == sender_id {
            trade.1
        } else {
            trade.0
        };

        // Schedule push notification (fire-and-forget; errors are logged, not propagated)
        Self::trigger_push_notification(pool, other_user_id, trade_id, content).await;

        let message = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = ?")
            .bind(message_id)
            .fetch_one(pool)
            .await?;

        Ok(message)
    }

    /// Get paginated messages for a trade conversation.
    pub async fn get_messages(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
        pagination: &PaginationParams,
    ) -> Result<PaginatedResponse<Message>, ApiError> {
        // Validate user is participant
        Self::verify_participant(pool, trade_id, user_id).await?;

        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM messages WHERE trade_id = ?",
        )
        .bind(trade_id)
        .fetch_one(pool)
        .await?;

        let messages = sqlx::query_as::<_, Message>(
            r#"
            SELECT * FROM messages
            WHERE trade_id = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(trade_id)
        .bind(pagination.per_page())
        .bind(pagination.offset())
        .fetch_all(pool)
        .await?;

        Ok(PaginatedResponse::new(
            messages,
            pagination.page(),
            pagination.per_page(),
            total.0 as u64,
        ))
    }

    /// Mark all unread messages in a conversation as read for the given user.
    pub async fn mark_messages_read(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<u64, ApiError> {
        Self::verify_participant(pool, trade_id, user_id).await?;

        let result = sqlx::query(
            r#"
            UPDATE messages
            SET is_read = 1, updated_at = NOW()
            WHERE trade_id = ?
              AND sender_id != ?
              AND is_read = 0
            "#,
        )
        .bind(trade_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get the inbox for a user -- a list of conversations with the last
    /// message and unread count.
    pub async fn get_inbox(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<InboxEntry>, ApiError> {
        let entries = sqlx::query_as::<_, InboxEntry>(
            r#"
            SELECT
                t.id AS trade_id,
                CASE WHEN t.initiator_id = ? THEN t.recipient_id ELSE t.initiator_id END AS other_user_id,
                u.username AS other_username,
                u.avatar_id AS other_avatar_id,
                (
                    SELECT m.content FROM messages m
                    WHERE m.trade_id = t.id
                    ORDER BY m.created_at DESC LIMIT 1
                ) AS last_message_content,
                (
                    SELECT m.created_at FROM messages m
                    WHERE m.trade_id = t.id
                    ORDER BY m.created_at DESC LIMIT 1
                ) AS last_message_at,
                (
                    SELECT COUNT(*) FROM messages m
                    WHERE m.trade_id = t.id AND m.sender_id != ? AND m.is_read = 0
                ) AS unread_count,
                t.status AS trade_status
            FROM trades t
            INNER JOIN users u ON u.id = CASE WHEN t.initiator_id = ? THEN t.recipient_id ELSE t.initiator_id END
            WHERE (t.initiator_id = ? OR t.recipient_id = ?)
              AND EXISTS (SELECT 1 FROM messages m WHERE m.trade_id = t.id)
            ORDER BY last_message_at DESC
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(entries)
    }

    /// Get total unread message count for a user across all conversations.
    pub async fn get_unread_count(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<i64, ApiError> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM messages m
            INNER JOIN trades t ON t.id = m.trade_id
            WHERE (t.initiator_id = ? OR t.recipient_id = ?)
              AND m.sender_id != ?
              AND m.is_read = 0
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(count.0)
    }

    /// Start a chat/conversation, optionally linking a book as a trade item.
    /// Returns (trade_id, was_created).
    pub async fn start_chat(
        pool: &MySqlPool,
        user_id: i64,
        other_user_id: i64,
        book_id: Option<i64>,
    ) -> Result<(i64, bool), ApiError> {
        if user_id == other_user_id {
            return Err(ApiError::bad_request("chat.cannot_chat_with_self"));
        }

        // Check for existing trade/conversation
        let existing: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM trades
            WHERE (
                (initiator_id = ? AND recipient_id = ?)
                OR (initiator_id = ? AND recipient_id = ?)
            )
              AND status NOT IN ('completed', 'cancelled', 'rejected')
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(other_user_id)
        .bind(other_user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        if let Some((id,)) = existing {
            return Ok((id, false));
        }

        let now = Utc::now().naive_utc();
        let result = sqlx::query(
            "INSERT INTO trades (initiator_id, recipient_id, status, created_at, updated_at)
             VALUES (?, ?, 'pending', ?, ?)",
        )
        .bind(user_id)
        .bind(other_user_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let trade_id = result.last_insert_id() as i64;

        if let Some(bid) = book_id {
            sqlx::query(
                "INSERT INTO trade_items (trade_id, book_id, owner_id)
                 SELECT ?, ?, user_id FROM books WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(trade_id)
            .bind(bid)
            .bind(bid)
            .execute(pool)
            .await?;
        }

        Ok((trade_id, true))
    }

    /// Get paginated inbox for a user with user info, last message, and unread counts.
    pub async fn get_inbox_paginated(
        pool: &MySqlPool,
        user_id: i64,
        pagination: &PaginationParams,
    ) -> Result<PaginatedResponse<InboxEntry>, ApiError> {
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE (initiator_id = ? OR recipient_id = ?)",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let entries = sqlx::query_as::<_, InboxEntry>(
            r#"
            SELECT
                t.id AS trade_id,
                CASE WHEN t.initiator_id = ? THEN t.recipient_id ELSE t.initiator_id END AS other_user_id,
                u.username AS other_username,
                u.avatar_id AS other_avatar_id,
                (
                    SELECT m.content FROM messages m
                    WHERE m.trade_id = t.id
                    ORDER BY m.created_at DESC LIMIT 1
                ) AS last_message_content,
                (
                    SELECT m.created_at FROM messages m
                    WHERE m.trade_id = t.id
                    ORDER BY m.created_at DESC LIMIT 1
                ) AS last_message_at,
                (
                    SELECT COUNT(*) FROM messages m
                    WHERE m.trade_id = t.id AND m.sender_id != ? AND m.is_read = 0
                ) AS unread_count,
                t.status AS trade_status
            FROM trades t
            INNER JOIN users u ON u.id = CASE WHEN t.initiator_id = ? THEN t.recipient_id ELSE t.initiator_id END
            WHERE (t.initiator_id = ? OR t.recipient_id = ?)
            ORDER BY t.updated_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(pagination.per_page())
        .bind(pagination.offset())
        .fetch_all(pool)
        .await?;

        Ok(PaginatedResponse::new(
            entries,
            pagination.page(),
            pagination.per_page(),
            total.0 as u64,
        ))
    }

    /// Send a system message (no sender, is_system_message = true).
    pub async fn send_system_message(
        pool: &MySqlPool,
        trade_id: i64,
        content: &str,
    ) -> Result<Message, ApiError> {
        let now = Utc::now().naive_utc();

        let result = sqlx::query(
            r#"
            INSERT INTO messages (
                trade_id, sender_id, content, `type`, status,
                is_read, is_system_message, push_sent,
                created_at, updated_at
            ) VALUES (?, NULL, ?, 'system', 'sent', 0, 1, 0, ?, ?)
            "#,
        )
        .bind(trade_id)
        .bind(content)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let message_id = result.last_insert_id() as i64;
        let message = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = ?")
            .bind(message_id)
            .fetch_one(pool)
            .await?;

        Ok(message)
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    async fn verify_participant(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<(), ApiError> {
        let trade: (i64, i64) = sqlx::query_as(
            "SELECT initiator_id, recipient_id FROM trades WHERE id = ?",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("trade.not_found"))?;

        if trade.0 != user_id && trade.1 != user_id {
            return Err(ApiError::forbidden());
        }
        Ok(())
    }

    /// Trigger a push notification for a new message. Errors are logged but
    /// do not fail the message send.
    async fn trigger_push_notification(
        pool: &MySqlPool,
        recipient_user_id: i64,
        trade_id: i64,
        _content: &str,
    ) {
        // Fetch active device tokens for the recipient
        let tokens: Vec<(String,)> = sqlx::query_as(
            "SELECT fcm_token FROM device_tokens WHERE user_id = ? AND is_active = 1",
        )
        .bind(recipient_user_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        if tokens.is_empty() {
            return;
        }

        // Mark push as queued -- actual FCM delivery is handled by the
        // NotificationService. We just record the intent here.
        let _ = sqlx::query(
            r#"
            UPDATE messages
            SET push_sent = 1, push_sent_at = NOW(), updated_at = NOW()
            WHERE trade_id = ?
              AND id = (SELECT id FROM (SELECT id FROM messages WHERE trade_id = ? ORDER BY created_at DESC LIMIT 1) AS tmp)
            "#,
        )
        .bind(trade_id)
        .bind(trade_id)
        .execute(pool)
        .await;
    }
}
