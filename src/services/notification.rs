use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::config::Config;
use crate::errors::ApiError;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub body: String,
    pub notification_type: String,
    pub data: Option<String>,
    pub read_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceToken {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub platform: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
struct FcmMessage {
    message: FcmMessageBody,
}

#[derive(Debug, Serialize)]
struct FcmMessageBody {
    token: String,
    notification: FcmNotification,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct FcmNotification {
    title: String,
    body: String,
}

// ── Notification Service ────────────────────────────────────────────────

pub struct NotificationService;

impl NotificationService {
    /// Create a notification record in the database.
    pub async fn create_notification(
        pool: &MySqlPool,
        user_id: i64,
        title: &str,
        body: &str,
        notification_type: &str,
        data: Option<&serde_json::Value>,
    ) -> Result<Notification, ApiError> {
        let data_json = data.map(|d| serde_json::to_string(d).unwrap_or_default());

        let result = sqlx::query(
            r#"
            INSERT INTO notifications (user_id, title, body, notification_type, data, created_at)
            VALUES (?, ?, ?, ?, ?, NOW())
            "#,
        )
        .bind(user_id)
        .bind(title)
        .bind(body)
        .bind(notification_type)
        .bind(&data_json)
        .execute(pool)
        .await?;

        let notification_id = result.last_insert_id() as i64;

        // Update the user's last_unread_notification_at timestamp.
        sqlx::query("UPDATE users SET last_unread_notification_at = NOW() WHERE id = ?")
            .bind(user_id)
            .execute(pool)
            .await?;

        let notification: Notification =
            sqlx::query_as("SELECT * FROM notifications WHERE id = ?")
                .bind(notification_id)
                .fetch_one(pool)
                .await?;

        Ok(notification)
    }

    /// Send a push notification to all of a user's registered devices via FCM.
    pub async fn send_push_notification(
        config: &Config,
        pool: &MySqlPool,
        user_id: i64,
        title: &str,
        body: &str,
        data: Option<&serde_json::Value>,
    ) -> Result<(), ApiError> {
        let tokens = Self::get_user_device_tokens(pool, user_id).await?;

        if tokens.is_empty() {
            tracing::debug!("No device tokens found for user {}", user_id);
            return Ok(());
        }

        let client = reqwest::Client::new();
        let fcm_url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            config.fcm_project_id
        );

        for device_token in &tokens {
            let message = FcmMessage {
                message: FcmMessageBody {
                    token: device_token.token.clone(),
                    notification: FcmNotification {
                        title: title.to_string(),
                        body: body.to_string(),
                    },
                    data: data.cloned(),
                },
            };

            let response = client
                .post(&fcm_url)
                .header("Authorization", format!("Bearer {}", config.fcm_server_key))
                .json(&message)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        tracing::warn!(
                            "FCM push failed for token {} (user {}): {} - {}",
                            device_token.token,
                            user_id,
                            status,
                            body_text
                        );

                        // If the token is invalid, remove it.
                        if status.as_u16() == 404 || status.as_u16() == 410 {
                            tracing::info!(
                                "Removing stale device token {} for user {}",
                                device_token.token,
                                user_id
                            );
                            let _ = sqlx::query("DELETE FROM device_tokens WHERE id = ?")
                                .bind(device_token.id)
                                .execute(pool)
                                .await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to send push to token {} (user {}): {:?}",
                        device_token.token,
                        user_id,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Get all device tokens for a user.
    pub async fn get_user_device_tokens(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<DeviceToken>, ApiError> {
        let tokens: Vec<DeviceToken> =
            sqlx::query_as("SELECT * FROM device_tokens WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(pool)
                .await?;

        Ok(tokens)
    }

    /// Mark a single notification as read.
    pub async fn mark_as_read(
        pool: &MySqlPool,
        notification_id: i64,
        user_id: i64,
    ) -> Result<bool, ApiError> {
        let result = sqlx::query(
            "UPDATE notifications SET read_at = NOW() WHERE id = ? AND user_id = ? AND read_at IS NULL",
        )
        .bind(notification_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark all notifications as read for a user.
    pub async fn mark_all_as_read(pool: &MySqlPool, user_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE notifications SET read_at = NOW() WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get the count of unread notifications for a user.
    pub async fn get_unread_count(pool: &MySqlPool, user_id: i64) -> Result<i64, ApiError> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// Get total notification count for a user.
    pub async fn get_total_count(pool: &MySqlPool, user_id: i64) -> Result<u64, ApiError> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(count as u64)
    }

    /// List notifications for a user with pagination.
    pub async fn list_notifications(
        pool: &MySqlPool,
        user_id: i64,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<crate::models::Notification>, ApiError> {
        let notifications = sqlx::query_as::<_, crate::models::Notification>(
            "SELECT * FROM notifications WHERE user_id = ?
             ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(notifications)
    }

    /// Get a single notification by id, verifying ownership.
    pub async fn get_notification(
        pool: &MySqlPool,
        notification_id: i64,
        user_id: i64,
    ) -> Result<crate::models::Notification, ApiError> {
        let notification = sqlx::query_as::<_, crate::models::Notification>(
            "SELECT * FROM notifications WHERE id = ? AND user_id = ?",
        )
        .bind(notification_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("notifications.not_found"))?;

        Ok(notification)
    }

    /// Register a device token for push notifications (upsert).
    pub async fn register_device(
        pool: &MySqlPool,
        user_id: i64,
        fcm_token: &str,
        device_type: &str,
    ) -> Result<(), ApiError> {
        let now = chrono::Utc::now().naive_utc();

        sqlx::query(
            "INSERT INTO device_tokens (user_id, fcm_token, device_type, is_active, last_used_at,
             created_at, updated_at)
             VALUES (?, ?, ?, true, ?, ?, ?)
             ON DUPLICATE KEY UPDATE is_active = true, last_used_at = VALUES(last_used_at),
             updated_at = VALUES(updated_at), user_id = VALUES(user_id)",
        )
        .bind(user_id)
        .bind(fcm_token)
        .bind(device_type)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Unregister (deactivate) a device token.
    pub async fn unregister_device(
        pool: &MySqlPool,
        user_id: i64,
        fcm_token: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE device_tokens SET is_active = false, updated_at = NOW()
             WHERE user_id = ? AND fcm_token = ?",
        )
        .bind(user_id)
        .bind(fcm_token)
        .execute(pool)
        .await?;

        Ok(())
    }
}
