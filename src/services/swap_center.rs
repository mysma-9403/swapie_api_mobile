use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

use crate::errors::ApiError;
use crate::models::{Book, BookMatch};

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SwapEntry {
    pub book_match: BookMatch,
    pub owner_book: Book,
    pub interested_book: Option<Book>,
    pub other_user_id: i64,
    pub other_username: String,
    pub other_avatar_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SwapDetails {
    pub matches: Vec<BookMatch>,
    pub your_books: Vec<Book>,
    pub their_books: Vec<Book>,
    pub other_user_id: i64,
    pub other_username: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ActivityEntry {
    pub id: i64,
    pub activity_type: String,
    pub related_id: i64,
    pub other_user_id: Option<i64>,
    pub other_username: Option<String>,
    pub title: Option<String>,
    pub created_at: NaiveDateTime,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct SwapCenterService;

#[derive(Debug, Serialize)]
pub struct SwapCenterOverview {
    pub active_swaps: i64,
    pub pending_offers: i64,
    pub you_like_count: i64,
    pub others_like_count: i64,
    pub matches_count: i64,
}

impl SwapCenterService {
    /// Get the swap center overview counts.
    pub async fn get_overview(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<SwapCenterOverview, ApiError> {
        let (active_swaps,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades
             WHERE (initiator_id = ? OR recipient_id = ?)
             AND status IN ('pending', 'accepted', 'awaiting_shipment', 'shipped')",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let (pending_offers,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE recipient_id = ? AND status = 'pending'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let (you_like_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM swipes WHERE user_id = ? AND (type = 'like' OR type = 'superlike')",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let (others_like_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT s.user_id) FROM swipes s
             INNER JOIN books b ON b.id = s.book_id
             WHERE b.user_id = ? AND (s.type = 'like' OR s.type = 'superlike')",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let (matches_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM book_matches
             WHERE (book_owner_id = ? OR interested_user_id = ?) AND status = 'pending'",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(SwapCenterOverview {
            active_swaps,
            pending_offers,
            you_like_count,
            others_like_count,
            matches_count,
        })
    }

    /// Get active trades for the swap center.
    pub async fn get_active_trades(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<crate::models::Trade>, ApiError> {
        let trades = sqlx::query_as::<_, crate::models::Trade>(
            "SELECT * FROM trades
             WHERE (initiator_id = ? OR recipient_id = ?)
             AND status IN ('pending', 'accepted', 'awaiting_shipment', 'shipped', 'countered')
             ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(trades)
    }

    /// Get books that the user liked (swiped right on).
    pub async fn get_liked_books(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<crate::models::Book>, ApiError> {
        let books = sqlx::query_as::<_, crate::models::Book>(
            "SELECT b.* FROM books b
             INNER JOIN swipes s ON s.book_id = b.id
             WHERE s.user_id = ? AND (s.type = 'like' OR s.type = 'superlike')
             AND b.deleted_at IS NULL
             ORDER BY s.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(books)
    }

    /// Get books belonging to the user that others liked.
    pub async fn get_books_others_liked(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<crate::models::Book>, ApiError> {
        let books = sqlx::query_as::<_, crate::models::Book>(
            "SELECT b.* FROM books b
             INNER JOIN swipes s ON s.book_id = b.id
             WHERE b.user_id = ? AND (s.type = 'like' OR s.type = 'superlike')
             AND b.deleted_at IS NULL
             ORDER BY s.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(books)
    }

    /// Get user info by id (lightweight, for swap summaries).
    pub async fn get_user_info(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Option<(i64, String, String, String, Option<i64>)>, ApiError> {
        let user = sqlx::query_as::<_, (i64, String, String, String, Option<i64>)>(
            "SELECT id, username, first_name, last_name, avatar_id FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Get active books for a user.
    pub async fn get_user_active_books(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<crate::models::Book>, ApiError> {
        let books = sqlx::query_as::<_, crate::models::Book>(
            "SELECT * FROM books WHERE user_id = ? AND deleted_at IS NULL AND status = 'active'",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(books)
    }

    /// Get mutual matches -- both users liked each other's books.
    pub async fn get_swaps(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<SwapEntry>, ApiError> {
        let matches = sqlx::query_as::<_, BookMatch>(
            r#"
            SELECT bm.* FROM book_matches bm
            WHERE (bm.book_owner_id = ? OR bm.interested_user_id = ?)
              AND bm.status = 'pending'
              AND bm.`type` = 'exchange'
              AND bm.matched_at IS NOT NULL
            ORDER BY bm.matched_at DESC
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let mut entries = Vec::with_capacity(matches.len());
        for m in matches {
            let other_user_id = if m.book_owner_id == user_id {
                m.interested_user_id
            } else {
                m.book_owner_id
            };

            let other_user: (String, Option<i64>) = sqlx::query_as(
                "SELECT username, avatar_id FROM users WHERE id = ?",
            )
            .bind(other_user_id)
            .fetch_one(pool)
            .await?;

            let owner_book = sqlx::query_as::<_, Book>(
                "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(m.owner_book_id)
            .fetch_optional(pool)
            .await?;

            let interested_book = if let Some(ib_id) = m.interested_book_id {
                sqlx::query_as::<_, Book>(
                    "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(ib_id)
                .fetch_optional(pool)
                .await?
            } else {
                None
            };

            if let Some(ob) = owner_book {
                entries.push(SwapEntry {
                    book_match: m,
                    owner_book: ob,
                    interested_book,
                    other_user_id,
                    other_username: other_user.0,
                    other_avatar_id: other_user.1,
                });
            }
        }

        Ok(entries)
    }

    /// Get books/users that liked YOUR books (others -> you).
    pub async fn get_you_like(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<SwapEntry>, ApiError> {
        let matches = sqlx::query_as::<_, BookMatch>(
            r#"
            SELECT bm.* FROM book_matches bm
            WHERE bm.book_owner_id = ?
              AND bm.status = 'pending'
            ORDER BY bm.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Self::hydrate_entries(pool, &matches, user_id).await
    }

    /// Get books that YOU liked (you -> others).
    pub async fn get_others_like(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<SwapEntry>, ApiError> {
        let matches = sqlx::query_as::<_, BookMatch>(
            r#"
            SELECT bm.* FROM book_matches bm
            WHERE bm.interested_user_id = ?
              AND bm.status = 'pending'
            ORDER BY bm.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Self::hydrate_entries(pool, &matches, user_id).await
    }

    /// Get detailed match information between two specific users.
    pub async fn get_swap_details(
        pool: &MySqlPool,
        user_id: i64,
        other_user_id: i64,
    ) -> Result<SwapDetails, ApiError> {
        let matches = sqlx::query_as::<_, BookMatch>(
            r#"
            SELECT * FROM book_matches
            WHERE (
                (book_owner_id = ? AND interested_user_id = ?)
                OR (book_owner_id = ? AND interested_user_id = ?)
            )
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(other_user_id)
        .bind(other_user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let your_books = sqlx::query_as::<_, Book>(
            r#"
            SELECT b.* FROM books b
            INNER JOIN swipes s ON s.book_id = b.id
            WHERE s.user_id = ?
              AND b.user_id = ?
              AND s.`type` IN ('like', 'superlike')
              AND b.deleted_at IS NULL
            "#,
        )
        .bind(other_user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let their_books = sqlx::query_as::<_, Book>(
            r#"
            SELECT b.* FROM books b
            INNER JOIN swipes s ON s.book_id = b.id
            WHERE s.user_id = ?
              AND b.user_id = ?
              AND s.`type` IN ('like', 'superlike')
              AND b.deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(other_user_id)
        .fetch_all(pool)
        .await?;

        let other_user: (String,) = sqlx::query_as(
            "SELECT username FROM users WHERE id = ?",
        )
        .bind(other_user_id)
        .fetch_one(pool)
        .await?;

        Ok(SwapDetails {
            matches,
            your_books,
            their_books,
            other_user_id,
            other_username: other_user.0,
        })
    }

    /// Get recent activity for a user (matches, offers, trade completions).
    pub async fn get_activity(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<ActivityEntry>, ApiError> {
        let activities = sqlx::query_as::<_, ActivityEntry>(
            r#"
            (
                SELECT
                    bm.id,
                    'match' AS activity_type,
                    bm.id AS related_id,
                    CASE WHEN bm.book_owner_id = ? THEN bm.interested_user_id ELSE bm.book_owner_id END AS other_user_id,
                    u.username AS other_username,
                    b.title AS title,
                    bm.created_at
                FROM book_matches bm
                INNER JOIN users u ON u.id = CASE WHEN bm.book_owner_id = ? THEN bm.interested_user_id ELSE bm.book_owner_id END
                LEFT JOIN books b ON b.id = bm.owner_book_id
                WHERE bm.book_owner_id = ? OR bm.interested_user_id = ?
            )
            UNION ALL
            (
                SELECT
                    t.id,
                    CONCAT('trade_', t.status) AS activity_type,
                    t.id AS related_id,
                    CASE WHEN t.initiator_id = ? THEN t.recipient_id ELSE t.initiator_id END AS other_user_id,
                    u.username AS other_username,
                    NULL AS title,
                    t.updated_at AS created_at
                FROM trades t
                INNER JOIN users u ON u.id = CASE WHEN t.initiator_id = ? THEN t.recipient_id ELSE t.initiator_id END
                WHERE t.initiator_id = ? OR t.recipient_id = ?
            )
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(activities)
    }

    /// Get all match pairs for a user.
    pub async fn get_matches(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<BookMatch>, ApiError> {
        let matches = sqlx::query_as::<_, BookMatch>(
            r#"
            SELECT * FROM book_matches
            WHERE (book_owner_id = ? OR interested_user_id = ?)
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(matches)
    }

    /// Get match details with a specific other user.
    pub async fn get_match_details(
        pool: &MySqlPool,
        user_id: i64,
        other_user_id: i64,
    ) -> Result<Vec<BookMatch>, ApiError> {
        let matches = sqlx::query_as::<_, BookMatch>(
            r#"
            SELECT * FROM book_matches
            WHERE (
                (book_owner_id = ? AND interested_user_id = ?)
                OR (book_owner_id = ? AND interested_user_id = ?)
            )
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(other_user_id)
        .bind(other_user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(matches)
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    async fn hydrate_entries(
        pool: &MySqlPool,
        matches: &[BookMatch],
        user_id: i64,
    ) -> Result<Vec<SwapEntry>, ApiError> {
        let mut entries = Vec::with_capacity(matches.len());

        for m in matches {
            let other_user_id = if m.book_owner_id == user_id {
                m.interested_user_id
            } else {
                m.book_owner_id
            };

            let other_user: (String, Option<i64>) = sqlx::query_as(
                "SELECT username, avatar_id FROM users WHERE id = ?",
            )
            .bind(other_user_id)
            .fetch_one(pool)
            .await?;

            let owner_book = sqlx::query_as::<_, Book>(
                "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(m.owner_book_id)
            .fetch_optional(pool)
            .await?;

            let interested_book = if let Some(ib_id) = m.interested_book_id {
                sqlx::query_as::<_, Book>(
                    "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(ib_id)
                .fetch_optional(pool)
                .await?
            } else {
                None
            };

            if let Some(ob) = owner_book {
                entries.push(SwapEntry {
                    book_match: m.clone(),
                    owner_book: ob,
                    interested_book,
                    other_user_id,
                    other_username: other_user.0,
                    other_avatar_id: other_user.1,
                });
            }
        }

        Ok(entries)
    }
}
