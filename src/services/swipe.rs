use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::errors::ApiError;
use crate::models::{Book, BookImage, BookMatch, Swipe};

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SwipeCandidate {
    pub book: Book,
    pub distance_km: Option<f64>,
    pub matching_genres: i64,
    pub matching_tags: i64,
}

#[derive(Debug, Serialize)]
pub struct SwipeResult {
    pub swipe: Swipe,
    pub is_match: bool,
    pub match_record: Option<BookMatch>,
}

#[derive(Debug, Serialize)]
pub struct SwipeUserInfo {
    pub id: i64,
    pub username: String,
    pub first_name: String,
    pub avatar_id: Option<i64>,
    pub average_rating: Option<rust_decimal::Decimal>,
    pub review_count: i32,
}

#[derive(Debug, Serialize)]
pub struct SwipeBookDetail {
    #[serde(flatten)]
    pub book: Book,
    pub images: Vec<BookImage>,
    pub user: SwipeUserInfo,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct SwipeService;

impl SwipeService {
    /// Get the next book candidate to swipe on.
    ///
    /// Excludes the user's own books, already-swiped books, books from blocked
    /// users, and inactive books. Prefers books with matching genres/tags and
    /// nearby location using haversine distance in SQL.
    pub async fn get_next_candidate(
        pool: &MySqlPool,
        user_id: i64,
        user_lat: Option<f64>,
        user_lng: Option<f64>,
    ) -> Result<Option<SwipeCandidate>, ApiError> {
        // Build the query with optional geo-distance scoring
        let row = sqlx::query_as::<_, Book>(
            r#"
            SELECT b.* FROM books b
            WHERE b.deleted_at IS NULL
              AND b.status = 'active'
              AND b.user_id != ?
              AND b.id NOT IN (
                  SELECT s.book_id FROM swipes s WHERE s.user_id = ?
              )
              AND b.user_id NOT IN (
                  SELECT blocked_user_id FROM user_blocks WHERE user_id = ?
                  UNION
                  SELECT user_id FROM user_blocks WHERE blocked_user_id = ?
              )
            ORDER BY
              (
                  SELECT COUNT(*) FROM book_genre bg
                  WHERE bg.book_id = b.id
                    AND bg.genre_id IN (
                        SELECT bg2.genre_id FROM book_genre bg2
                        INNER JOIN books b2 ON b2.id = bg2.book_id AND b2.user_id = ? AND b2.deleted_at IS NULL
                    )
              ) DESC,
              (
                  SELECT COUNT(*) FROM book_tag bt
                  WHERE bt.book_id = b.id
                    AND bt.tag_id IN (
                        SELECT bt2.tag_id FROM book_tag bt2
                        INNER JOIN books b2 ON b2.id = bt2.book_id AND b2.user_id = ? AND b2.deleted_at IS NULL
                    )
              ) DESC,
              CASE
                  WHEN b.latitude IS NOT NULL AND b.longitude IS NOT NULL AND ? IS NOT NULL AND ? IS NOT NULL THEN
                      (6371 * ACOS(
                          LEAST(1.0, COS(RADIANS(?)) * COS(RADIANS(b.latitude))
                          * COS(RADIANS(b.longitude) - RADIANS(?))
                          + SIN(RADIANS(?)) * SIN(RADIANS(b.latitude)))
                      ))
                  ELSE 999999
              END ASC,
              b.likes_count DESC,
              b.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_lat)
        .bind(user_lng)
        .bind(user_lat)
        .bind(user_lng)
        .bind(user_lat)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(book) => {
                // Calculate distance if coordinates available
                let distance_km = match (user_lat, user_lng, &book.latitude, &book.longitude) {
                    (Some(ulat), Some(ulng), Some(blat), Some(blng)) => {
                        let blat_f = blat.to_string().parse::<f64>().unwrap_or(0.0);
                        let blng_f = blng.to_string().parse::<f64>().unwrap_or(0.0);
                        Some(haversine_km(ulat, ulng, blat_f, blng_f))
                    }
                    _ => None,
                };

                // Count matching genres
                let matching_genres: (i64,) = sqlx::query_as(
                    r#"
                    SELECT COUNT(*) FROM book_genre bg
                    WHERE bg.book_id = ?
                      AND bg.genre_id IN (
                          SELECT bg2.genre_id FROM book_genre bg2
                          INNER JOIN books b2 ON b2.id = bg2.book_id AND b2.user_id = ? AND b2.deleted_at IS NULL
                      )
                    "#,
                )
                .bind(book.id)
                .bind(user_id)
                .fetch_one(pool)
                .await?;

                // Count matching tags
                let matching_tags: (i64,) = sqlx::query_as(
                    r#"
                    SELECT COUNT(*) FROM book_tag bt
                    WHERE bt.book_id = ?
                      AND bt.tag_id IN (
                          SELECT bt2.tag_id FROM book_tag bt2
                          INNER JOIN books b2 ON b2.id = bt2.book_id AND b2.user_id = ? AND b2.deleted_at IS NULL
                      )
                    "#,
                )
                .bind(book.id)
                .bind(user_id)
                .fetch_one(pool)
                .await?;

                Ok(Some(SwipeCandidate {
                    book,
                    distance_km,
                    matching_genres: matching_genres.0,
                    matching_tags: matching_tags.0,
                }))
            }
            None => Ok(None),
        }
    }

    /// Handle a swipe action (like, superlike, reject). Creates the swipe
    /// record and checks for a match if the action is a like or superlike.
    pub async fn handle_swipe(
        pool: &MySqlPool,
        user_id: i64,
        book_id: i64,
        swipe_type: &str,
    ) -> Result<SwipeResult, ApiError> {
        // Validate the book exists and is active
        let book = sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL AND status = 'active'",
        )
        .bind(book_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("book.not_found"))?;

        if book.user_id == user_id {
            return Err(ApiError::bad_request("swipe.cannot_swipe_own_book"));
        }

        // Check for existing swipe
        let existing: Option<Swipe> = sqlx::query_as(
            "SELECT * FROM swipes WHERE user_id = ? AND book_id = ?",
        )
        .bind(user_id)
        .bind(book_id)
        .fetch_optional(pool)
        .await?;

        if existing.is_some() {
            return Err(ApiError::conflict("swipe.already_swiped"));
        }

        let now = Utc::now().naive_utc();

        // Create swipe record
        let result = sqlx::query(
            "INSERT INTO swipes (user_id, book_id, `type`, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(book_id)
        .bind(swipe_type)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let swipe_id = result.last_insert_id() as i64;

        let swipe = sqlx::query_as::<_, Swipe>("SELECT * FROM swipes WHERE id = ?")
            .bind(swipe_id)
            .fetch_one(pool)
            .await?;

        // Update likes count
        if swipe_type == "like" || swipe_type == "superlike" {
            sqlx::query(
                "UPDATE books SET likes_count = likes_count + 1, updated_at = NOW() WHERE id = ?",
            )
            .bind(book_id)
            .execute(pool)
            .await?;
        }

        // Check for match on likes / superlikes
        let mut is_match = false;
        let mut match_record = None;

        if swipe_type == "like" || swipe_type == "superlike" {
            if let Some(m) = Self::check_for_match(pool, user_id, book_id).await? {
                is_match = true;
                match_record = Some(m);
            }

            // Purchase match: superlike on a for_sale book
            if swipe_type == "superlike" && book.for_sale && !is_match {
                let purchase_match = Self::create_purchase_match(pool, user_id, &book).await?;
                is_match = true;
                match_record = Some(purchase_match);
            }
        }

        Ok(SwipeResult {
            swipe,
            is_match,
            match_record,
        })
    }

    /// Toggle a swipe between like and unlike (removes if already liked,
    /// creates like if previously rejected or not yet swiped).
    pub async fn toggle_swipe(
        pool: &MySqlPool,
        user_id: i64,
        book_id: i64,
    ) -> Result<Option<Swipe>, ApiError> {
        let existing: Option<Swipe> = sqlx::query_as(
            "SELECT * FROM swipes WHERE user_id = ? AND book_id = ?",
        )
        .bind(user_id)
        .bind(book_id)
        .fetch_optional(pool)
        .await?;

        match existing {
            Some(ref s) if s.swipe_type.to_string() == "like" || s.swipe_type.to_string() == "superlike" => {
                // Remove the like
                sqlx::query("DELETE FROM swipes WHERE id = ?")
                    .bind(s.id)
                    .execute(pool)
                    .await?;
                sqlx::query(
                    "UPDATE books SET likes_count = GREATEST(likes_count - 1, 0), updated_at = NOW() WHERE id = ?",
                )
                .bind(book_id)
                .execute(pool)
                .await?;
                Ok(None)
            }
            Some(ref s) => {
                // Was a reject, change to like
                let now = Utc::now().naive_utc();
                sqlx::query(
                    "UPDATE swipes SET `type` = 'like', updated_at = ? WHERE id = ?",
                )
                .bind(now)
                .bind(s.id)
                .execute(pool)
                .await?;
                sqlx::query(
                    "UPDATE books SET likes_count = likes_count + 1, updated_at = NOW() WHERE id = ?",
                )
                .bind(book_id)
                .execute(pool)
                .await?;
                let updated = sqlx::query_as::<_, Swipe>("SELECT * FROM swipes WHERE id = ?")
                    .bind(s.id)
                    .fetch_one(pool)
                    .await?;
                Ok(Some(updated))
            }
            None => {
                // No existing swipe -- create a like
                let now = Utc::now().naive_utc();
                let result = sqlx::query(
                    "INSERT INTO swipes (user_id, book_id, `type`, created_at, updated_at) VALUES (?, ?, 'like', ?, ?)",
                )
                .bind(user_id)
                .bind(book_id)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(
                    "UPDATE books SET likes_count = likes_count + 1, updated_at = NOW() WHERE id = ?",
                )
                .bind(book_id)
                .execute(pool)
                .await?;
                let swipe = sqlx::query_as::<_, Swipe>("SELECT * FROM swipes WHERE id = ?")
                    .bind(result.last_insert_id() as i64)
                    .fetch_one(pool)
                    .await?;
                Ok(Some(swipe))
            }
        }
    }

    /// Get the next book to swipe on, with images and user info.
    /// Returns None if there are no more books to swipe on.
    pub async fn get_next_swipe_detail(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Option<SwipeBookDetail>, ApiError> {
        let book = sqlx::query_as::<_, Book>(
            "SELECT b.* FROM books b
             WHERE b.deleted_at IS NULL
             AND b.status = 'active'
             AND b.user_id != ?
             AND b.id NOT IN (SELECT book_id FROM swipes WHERE user_id = ?)
             AND b.user_id NOT IN (SELECT blocked_id FROM user_blocks WHERE blocker_id = ?)
             AND b.user_id NOT IN (SELECT blocker_id FROM user_blocks WHERE blocked_id = ?)
             ORDER BY b.created_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        let book = match book {
            Some(b) => b,
            None => return Ok(None),
        };

        let images = sqlx::query_as::<_, BookImage>(
            "SELECT * FROM book_images WHERE book_id = ? ORDER BY `order` ASC",
        )
        .bind(book.id)
        .fetch_all(pool)
        .await?;

        let user = sqlx::query_as::<_, (i64, String, String, Option<i64>, Option<rust_decimal::Decimal>, i32)>(
            "SELECT id, username, first_name, avatar_id, average_rating, review_count
             FROM users WHERE id = ?",
        )
        .bind(book.user_id)
        .fetch_one(pool)
        .await?;

        Ok(Some(SwipeBookDetail {
            book,
            images,
            user: SwipeUserInfo {
                id: user.0,
                username: user.1,
                first_name: user.2,
                avatar_id: user.3,
                average_rating: user.4,
                review_count: user.5,
            },
        }))
    }

    /// Check if a mutual like exists between the current user and the book
    /// owner. If user A liked one of user B's books AND user B liked one of
    /// user A's books, a BookMatch is created.
    pub async fn check_for_match(
        pool: &MySqlPool,
        user_id: i64,
        book_id: i64,
    ) -> Result<Option<BookMatch>, ApiError> {
        // Get the book owner
        let book = sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(book_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("book.not_found"))?;

        let book_owner_id = book.user_id;
        if book_owner_id == user_id {
            return Ok(None);
        }

        // Check if the book owner has liked any of the current user's books
        let mutual: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT s.book_id FROM swipes s
            INNER JOIN books b ON b.id = s.book_id
            WHERE s.user_id = ?
              AND b.user_id = ?
              AND s.`type` IN ('like', 'superlike')
              AND b.deleted_at IS NULL
              AND b.status = 'active'
            LIMIT 1
            "#,
        )
        .bind(book_owner_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        match mutual {
            Some((interested_book_id,)) => {
                // Check if this match already exists
                let existing_match: Option<BookMatch> = sqlx::query_as(
                    r#"
                    SELECT * FROM book_matches
                    WHERE (
                        (book_owner_id = ? AND interested_user_id = ? AND owner_book_id = ?)
                        OR (book_owner_id = ? AND interested_user_id = ? AND owner_book_id = ?)
                    )
                    LIMIT 1
                    "#,
                )
                .bind(book_owner_id)
                .bind(user_id)
                .bind(book_id)
                .bind(user_id)
                .bind(book_owner_id)
                .bind(interested_book_id)
                .fetch_optional(pool)
                .await?;

                if existing_match.is_some() {
                    return Ok(existing_match);
                }

                // Create the match
                let now = Utc::now().naive_utc();
                let result = sqlx::query(
                    r#"
                    INSERT INTO book_matches (
                        book_owner_id, interested_user_id, owner_book_id, interested_book_id,
                        `type`, status, matched_at, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, 'exchange', 'pending', ?, ?, ?)
                    "#,
                )
                .bind(book_owner_id)
                .bind(user_id)
                .bind(book_id)
                .bind(interested_book_id)
                .bind(now)
                .bind(now)
                .bind(now)
                .execute(pool)
                .await?;

                let match_id = result.last_insert_id() as i64;
                let book_match = sqlx::query_as::<_, BookMatch>(
                    "SELECT * FROM book_matches WHERE id = ?",
                )
                .bind(match_id)
                .fetch_one(pool)
                .await?;

                Ok(Some(book_match))
            }
            None => Ok(None),
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    /// Create a purchase-type match when a user superlikes a for_sale book.
    async fn create_purchase_match(
        pool: &MySqlPool,
        user_id: i64,
        book: &Book,
    ) -> Result<BookMatch, ApiError> {
        let now = Utc::now().naive_utc();

        let result = sqlx::query(
            r#"
            INSERT INTO book_matches (
                book_owner_id, interested_user_id, owner_book_id, interested_book_id,
                `type`, status, matched_at, created_at, updated_at
            ) VALUES (?, ?, ?, NULL, 'purchase', 'pending', ?, ?, ?)
            "#,
        )
        .bind(book.user_id)
        .bind(user_id)
        .bind(book.id)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let match_id = result.last_insert_id() as i64;
        let book_match = sqlx::query_as::<_, BookMatch>(
            "SELECT * FROM book_matches WHERE id = ?",
        )
        .bind(match_id)
        .fetch_one(pool)
        .await?;

        Ok(book_match)
    }
}

/// Haversine distance in kilometres between two lat/lng points.
fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let r = 6371.0; // Earth radius in km
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r * c
}
