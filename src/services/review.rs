use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::MySqlPool;

use crate::dto::{PaginatedResponse, PaginationParams};
use crate::errors::ApiError;
use crate::models::Review;

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ReviewStatus {
    pub can_review: bool,
    pub has_reviewed: bool,
    pub trade_completed: bool,
}

#[derive(Debug, Serialize)]
pub struct ReviewStatusFull {
    pub can_review: bool,
    pub has_reviewed: bool,
    pub other_has_reviewed: bool,
}

#[derive(Debug, Serialize)]
pub struct ReviewWithUser {
    pub review: Review,
    pub reviewer_username: String,
    pub reviewer_first_name: String,
    pub reviewer_avatar_id: Option<i64>,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct ReviewService;

impl ReviewService {
    /// Create a review for a completed trade.
    ///
    /// Validates that the trade is completed, the reviewer is a participant,
    /// and the reviewer has not already submitted a review for this trade.
    /// After creating the review, updates the reviewed user's average_rating
    /// and review_count.
    pub async fn create_review(
        pool: &MySqlPool,
        reviewer_id: i64,
        trade_id: i64,
        rating: i8,
        comment: Option<&str>,
    ) -> Result<Review, ApiError> {
        // Validate rating range
        if !(1..=5).contains(&rating) {
            return Err(ApiError::validation("review.invalid_rating"));
        }

        // Fetch the trade and validate status
        let trade: Option<(i64, i64, String)> = sqlx::query_as(
            "SELECT initiator_id, recipient_id, status FROM trades WHERE id = ?",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;

        let (initiator_id, recipient_id, status) =
            trade.ok_or_else(|| ApiError::not_found("trade.not_found"))?;

        if status != "completed" {
            return Err(ApiError::bad_request("review.trade_not_completed"));
        }

        // Validate reviewer is a participant
        if reviewer_id != initiator_id && reviewer_id != recipient_id {
            return Err(ApiError::forbidden());
        }

        // Determine the reviewed user
        let reviewed_user_id = if reviewer_id == initiator_id {
            recipient_id
        } else {
            initiator_id
        };

        // Check if already reviewed
        let existing: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE trade_id = ? AND reviewer_id = ?",
        )
        .bind(trade_id)
        .bind(reviewer_id)
        .fetch_one(pool)
        .await?;

        if existing.0 > 0 {
            return Err(ApiError::conflict("review.already_reviewed"));
        }

        let mut tx = pool.begin().await?;

        // Create the review
        let result = sqlx::query(
            r#"
            INSERT INTO reviews (
                trade_id, reviewer_id, reviewed_user_id, rating, comment,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, NOW(), NOW())
            "#,
        )
        .bind(trade_id)
        .bind(reviewer_id)
        .bind(reviewed_user_id)
        .bind(rating)
        .bind(comment)
        .execute(&mut *tx)
        .await?;

        let review_id = result.last_insert_id() as i64;

        // Recalculate average rating for the reviewed user
        let avg: (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(AVG(rating), 0) FROM reviews WHERE reviewed_user_id = ?",
        )
        .bind(reviewed_user_id)
        .fetch_one(&mut *tx)
        .await?;

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE reviewed_user_id = ?",
        )
        .bind(reviewed_user_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE users SET average_rating = ?, review_count = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(avg.0)
        .bind(count.0 as i32)
        .bind(reviewed_user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let review = sqlx::query_as::<_, Review>("SELECT * FROM reviews WHERE id = ?")
            .bind(review_id)
            .fetch_one(pool)
            .await?;

        Ok(review)
    }

    /// Check whether the user can review and whether they have already reviewed
    /// for a given trade.
    pub async fn get_review_status(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<ReviewStatus, ApiError> {
        let trade: Option<(i64, i64, String)> = sqlx::query_as(
            "SELECT initiator_id, recipient_id, status FROM trades WHERE id = ?",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;

        let (initiator_id, recipient_id, status) =
            trade.ok_or_else(|| ApiError::not_found("trade.not_found"))?;

        let is_participant = user_id == initiator_id || user_id == recipient_id;
        let trade_completed = status == "completed";

        let has_reviewed: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE trade_id = ? AND reviewer_id = ?",
        )
        .bind(trade_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(ReviewStatus {
            can_review: is_participant && trade_completed && has_reviewed.0 == 0,
            has_reviewed: has_reviewed.0 > 0,
            trade_completed,
        })
    }

    /// Get review status for a trade including whether the other party has reviewed.
    pub async fn get_review_status_full(
        pool: &MySqlPool,
        trade_id: i64,
        user_id: i64,
    ) -> Result<ReviewStatusFull, ApiError> {
        let trade: Option<(i64, i64, String)> = sqlx::query_as(
            "SELECT initiator_id, recipient_id, status FROM trades WHERE id = ?",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;

        let (initiator_id, recipient_id, status) =
            trade.ok_or_else(|| ApiError::not_found("trade.not_found"))?;

        let is_participant = user_id == initiator_id || user_id == recipient_id;
        if !is_participant {
            return Err(ApiError::not_found("reviews.trade_not_found"));
        }

        let trade_completed = status == "completed";

        let other_user_id = if user_id == initiator_id {
            recipient_id
        } else {
            initiator_id
        };

        let has_reviewed: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE trade_id = ? AND reviewer_id = ?",
        )
        .bind(trade_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let other_has_reviewed: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE trade_id = ? AND reviewer_id = ?",
        )
        .bind(trade_id)
        .bind(other_user_id)
        .fetch_one(pool)
        .await?;

        Ok(ReviewStatusFull {
            can_review: is_participant && trade_completed && has_reviewed.0 == 0,
            has_reviewed: has_reviewed.0 > 0,
            other_has_reviewed: other_has_reviewed.0 > 0,
        })
    }

    /// Get paginated reviews for a user, enriched with reviewer info.
    pub async fn get_user_reviews_enriched(
        pool: &MySqlPool,
        user_id: i64,
        pagination: &PaginationParams,
    ) -> Result<PaginatedResponse<ReviewWithUser>, ApiError> {
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE reviewed_user_id = ?",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let reviews = sqlx::query_as::<_, Review>(
            r#"
            SELECT * FROM reviews
            WHERE reviewed_user_id = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(user_id)
        .bind(pagination.per_page())
        .bind(pagination.offset())
        .fetch_all(pool)
        .await?;

        let mut enriched: Vec<ReviewWithUser> = Vec::new();
        for review in reviews {
            let reviewer = sqlx::query_as::<_, (String, String, Option<i64>)>(
                "SELECT username, first_name, avatar_id FROM users WHERE id = ?",
            )
            .bind(review.reviewer_id)
            .fetch_optional(pool)
            .await?;

            if let Some(r) = reviewer {
                enriched.push(ReviewWithUser {
                    review,
                    reviewer_username: r.0,
                    reviewer_first_name: r.1,
                    reviewer_avatar_id: r.2,
                });
            }
        }

        Ok(PaginatedResponse::new(
            enriched,
            pagination.page(),
            pagination.per_page(),
            total.0 as u64,
        ))
    }

    /// Get paginated reviews for a user (reviews where they are the reviewed party).
    pub async fn get_user_reviews(
        pool: &MySqlPool,
        user_id: i64,
        pagination: &PaginationParams,
    ) -> Result<PaginatedResponse<Review>, ApiError> {
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM reviews WHERE reviewed_user_id = ?",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let reviews = sqlx::query_as::<_, Review>(
            r#"
            SELECT * FROM reviews
            WHERE reviewed_user_id = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(user_id)
        .bind(pagination.per_page())
        .bind(pagination.offset())
        .fetch_all(pool)
        .await?;

        Ok(PaginatedResponse::new(
            reviews,
            pagination.page(),
            pagination.per_page(),
            total.0 as u64,
        ))
    }
}
