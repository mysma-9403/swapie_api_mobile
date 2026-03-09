use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};

use crate::errors::ApiError;
use crate::models::{Book, Genre, Tag, UserSafe};

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct FullProfile {
    #[serde(flatten)]
    pub user: UserSafe,
    pub address: Option<UserAddress>,
    pub stats: UserStats,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PublicProfile {
    pub id: i64,
    pub username: String,
    pub first_name: String,
    pub description: Option<String>,
    pub avatar_url: Option<String>,
    pub average_rating: Option<Decimal>,
    pub review_count: i32,
    pub books_count: i64,
    pub member_since: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAddress {
    pub id: i64,
    pub user_id: i64,
    pub street: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub state: Option<String>,
    pub latitude: Option<Decimal>,
    pub longitude: Option<Decimal>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct UserStats {
    pub books_count: i64,
    pub active_books_count: i64,
    pub trades_completed: i64,
    pub trades_active: i64,
    pub total_swipes: i64,
    pub matches_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ProfileDetails {
    pub user: UserSafe,
    pub address: Option<UserAddress>,
    pub books_count: i64,
    pub trades_count: i64,
    pub genres: Vec<Genre>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Serialize)]
pub struct PublicProfileWithBooks {
    pub user_id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub description: Option<String>,
    pub avatar_id: Option<i64>,
    pub average_rating: Option<Decimal>,
    pub review_count: i32,
    pub created_at: chrono::NaiveDateTime,
    pub books: Vec<Book>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileData {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub preferred_item_types: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAddressData {
    pub street: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub state: Option<String>,
    pub latitude: Option<Decimal>,
    pub longitude: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct BlockedUserInfo {
    pub block_id: i64,
    pub user_id: i64,
    pub username: String,
    pub first_name: String,
    pub avatar_id: Option<i64>,
    pub reason: Option<String>,
    pub blocked_at: chrono::NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLockerData {
    pub default_inpost_locker: Option<String>,
    pub locker_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GdprConsent {
    pub privacy_policy_accepted: bool,
    pub terms_of_service_accepted: bool,
    pub marketing_emails_accepted: bool,
    pub consents_accepted_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGdprConsentData {
    pub privacy_policy_accepted: Option<bool>,
    pub terms_of_service_accepted: Option<bool>,
    pub marketing_emails_accepted: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GdprExport {
    pub user: serde_json::Value,
    pub address: Option<serde_json::Value>,
    pub books: Vec<serde_json::Value>,
    pub trades: Vec<serde_json::Value>,
    pub messages: Vec<serde_json::Value>,
    pub swipes: Vec<serde_json::Value>,
    pub reviews: Vec<serde_json::Value>,
    pub payment_methods: Vec<serde_json::Value>,
    pub exported_at: String,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct ProfileService;

impl ProfileService {
    /// Get the full profile for the authenticated user, including address and stats.
    pub async fn get_profile(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<FullProfile, ApiError> {
        let user = sqlx::query_as::<_, UserSafe>(
            r#"
            SELECT
                id, email, username, first_name, last_name, description,
                phone_number, avatar_id, language, locker_id,
                stripe_customer_id, stripe_connect_account_id, stripe_connect_status,
                stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
                stripe_connect_onboarded_at,
                privacy_policy_accepted, terms_of_service_accepted,
                marketing_emails_accepted, consents_accepted_at,
                google_id, facebook_id, social_provider, social_provider_id,
                average_rating, review_count,
                activation_code_expires_at, last_unread_notification_at,
                preferred_item_types, default_inpost_locker,
                email_verified_at, created_at, updated_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        let address = sqlx::query_as::<_, UserAddress>(
            "SELECT * FROM user_addresses WHERE user_id = ? LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        let stats = Self::get_stats(pool, user_id).await?;

        let avatar_url = if let Some(avatar_id) = user.avatar_id {
            let path: Option<(String,)> = sqlx::query_as(
                "SELECT path FROM media WHERE id = ?",
            )
            .bind(avatar_id)
            .fetch_optional(pool)
            .await?;
            path.map(|p| p.0)
        } else {
            None
        };

        Ok(FullProfile {
            user,
            address,
            stats,
            avatar_url,
        })
    }

    /// Get a public profile visible to other users.
    pub async fn get_public_profile(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<PublicProfile, ApiError> {
        let user: (i64, String, String, Option<String>, Option<i64>, Option<Decimal>, i32, chrono::NaiveDateTime) =
            sqlx::query_as(
                r#"
                SELECT id, username, first_name, description, avatar_id,
                       average_rating, review_count, created_at
                FROM users WHERE id = ?
                "#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        let books_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM books WHERE user_id = ? AND deleted_at IS NULL AND status = 'active'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let avatar_url = if let Some(avatar_id) = user.4 {
            let path: Option<(String,)> = sqlx::query_as(
                "SELECT path FROM media WHERE id = ?",
            )
            .bind(avatar_id)
            .fetch_optional(pool)
            .await?;
            path.map(|p| p.0)
        } else {
            None
        };

        Ok(PublicProfile {
            id: user.0,
            username: user.1,
            first_name: user.2,
            description: user.3,
            avatar_url,
            average_rating: user.5,
            review_count: user.6,
            books_count: books_count.0,
            member_since: user.7,
        })
    }

    /// Get detailed profile for the authenticated user, including genres, tags, and counts.
    pub async fn get_profile_details(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<ProfileDetails, ApiError> {
        let user = sqlx::query_as::<_, UserSafe>(
            r#"
            SELECT
                id, email, username, first_name, last_name, description,
                phone_number, avatar_id, language, locker_id,
                stripe_customer_id, stripe_connect_account_id, stripe_connect_status,
                stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
                stripe_connect_onboarded_at,
                privacy_policy_accepted, terms_of_service_accepted,
                marketing_emails_accepted, consents_accepted_at,
                google_id, facebook_id, social_provider, social_provider_id,
                average_rating, review_count,
                activation_code_expires_at, last_unread_notification_at,
                preferred_item_types, default_inpost_locker,
                email_verified_at, created_at, updated_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        let address = sqlx::query_as::<_, UserAddress>(
            "SELECT * FROM user_addresses WHERE user_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        let books_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM books WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let trades_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE (initiator_id = ? OR recipient_id = ?)",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let genres = sqlx::query_as::<_, Genre>(
            "SELECT g.* FROM genres g
             INNER JOIN user_genres ug ON ug.genre_id = g.id
             WHERE ug.user_id = ?",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let tags = sqlx::query_as::<_, Tag>(
            "SELECT t.* FROM tags t
             INNER JOIN user_tags ut ON ut.tag_id = t.id
             WHERE ut.user_id = ?",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(ProfileDetails {
            user,
            address,
            books_count: books_count.0,
            trades_count: trades_count.0,
            genres,
            tags,
        })
    }

    /// Get a public user profile with their active books.
    pub async fn get_user_profile_with_books(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<PublicProfileWithBooks, ApiError> {
        let user = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<i64>, Option<Decimal>, i32, chrono::NaiveDateTime)>(
            "SELECT id, username, first_name, last_name, description, avatar_id, average_rating, review_count, created_at
             FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        let books = sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE user_id = ? AND deleted_at IS NULL AND status = 'active'
             ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(PublicProfileWithBooks {
            user_id: user.0,
            username: user.1,
            first_name: user.2,
            last_name: user.3,
            description: user.4,
            avatar_id: user.5,
            average_rating: user.6,
            review_count: user.7,
            created_at: user.8,
            books,
        })
    }

    /// Update profile fields, including optional genres and tags.
    pub async fn update_profile_with_relations(
        pool: &MySqlPool,
        user_id: i64,
        data: UpdateProfileData,
        genre_ids: Option<Vec<i64>>,
        tag_ids: Option<Vec<i64>>,
    ) -> Result<UserSafe, ApiError> {
        Self::update_profile(pool, user_id, data).await?;

        if let Some(genres) = genre_ids {
            sqlx::query("DELETE FROM user_genres WHERE user_id = ?")
                .bind(user_id)
                .execute(pool)
                .await?;
            for genre_id in genres {
                sqlx::query("INSERT INTO user_genres (user_id, genre_id) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(genre_id)
                    .execute(pool)
                    .await?;
            }
        }

        if let Some(tags) = tag_ids {
            sqlx::query("DELETE FROM user_tags WHERE user_id = ?")
                .bind(user_id)
                .execute(pool)
                .await?;
            for tag_id in tags {
                sqlx::query("INSERT INTO user_tags (user_id, tag_id) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(tag_id)
                    .execute(pool)
                    .await?;
            }
        }

        let user = sqlx::query_as::<_, UserSafe>(
            r#"
            SELECT
                id, email, username, first_name, last_name, description,
                phone_number, avatar_id, language, locker_id,
                stripe_customer_id, stripe_connect_account_id, stripe_connect_status,
                stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
                stripe_connect_onboarded_at,
                privacy_policy_accepted, terms_of_service_accepted,
                marketing_emails_accepted, consents_accepted_at,
                google_id, facebook_id, social_provider, social_provider_id,
                average_rating, review_count,
                activation_code_expires_at, last_unread_notification_at,
                preferred_item_types, default_inpost_locker,
                email_verified_at, created_at, updated_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Update the user's phone number, checking for uniqueness first.
    pub async fn update_phone_checked(
        pool: &MySqlPool,
        user_id: i64,
        phone: &str,
    ) -> Result<(), ApiError> {
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE phone_number = ? AND id != ?",
        )
        .bind(phone)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        if exists.0 > 0 {
            return Err(ApiError::conflict("auth.phone_already_in_use"));
        }

        Self::update_phone(pool, user_id, phone).await
    }

    /// Check for active trades before account deletion.
    pub async fn check_active_trades(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<bool, ApiError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades
             WHERE (initiator_id = ? OR recipient_id = ?)
             AND status IN ('pending', 'accepted', 'awaiting_shipment', 'shipped')",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(count.0 > 0)
    }

    /// Update profile fields.
    pub async fn update_profile(
        pool: &MySqlPool,
        user_id: i64,
        data: UpdateProfileData,
    ) -> Result<(), ApiError> {
        // Check username uniqueness if being changed
        if let Some(ref username) = data.username {
            let existing: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM users WHERE username = ? AND id != ?",
            )
            .bind(username)
            .bind(user_id)
            .fetch_one(pool)
            .await?;

            if existing.0 > 0 {
                return Err(ApiError::conflict("user.username_taken"));
            }
        }

        sqlx::query(
            r#"
            UPDATE users SET
                first_name = COALESCE(?, first_name),
                last_name = COALESCE(?, last_name),
                username = COALESCE(?, username),
                description = COALESCE(?, description),
                language = COALESCE(?, language),
                preferred_item_types = COALESCE(?, preferred_item_types),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(&data.first_name)
        .bind(&data.last_name)
        .bind(&data.username)
        .bind(&data.description)
        .bind(&data.language)
        .bind(&data.preferred_item_types)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Upsert the user's address.
    pub async fn update_address(
        pool: &MySqlPool,
        user_id: i64,
        data: UpdateAddressData,
    ) -> Result<UserAddress, ApiError> {
        let existing = sqlx::query_as::<_, UserAddress>(
            "SELECT * FROM user_addresses WHERE user_id = ? LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        let now = Utc::now().naive_utc();

        if existing.is_some() {
            sqlx::query(
                r#"
                UPDATE user_addresses SET
                    street = COALESCE(?, street),
                    city = COALESCE(?, city),
                    postal_code = COALESCE(?, postal_code),
                    country = COALESCE(?, country),
                    state = COALESCE(?, state),
                    latitude = COALESCE(?, latitude),
                    longitude = COALESCE(?, longitude),
                    updated_at = ?
                WHERE user_id = ?
                "#,
            )
            .bind(&data.street)
            .bind(&data.city)
            .bind(&data.postal_code)
            .bind(&data.country)
            .bind(&data.state)
            .bind(&data.latitude)
            .bind(&data.longitude)
            .bind(now)
            .bind(user_id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO user_addresses (
                    user_id, street, city, postal_code, country, state,
                    latitude, longitude, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(user_id)
            .bind(&data.street)
            .bind(&data.city)
            .bind(&data.postal_code)
            .bind(&data.country)
            .bind(&data.state)
            .bind(&data.latitude)
            .bind(&data.longitude)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
        }

        let address = sqlx::query_as::<_, UserAddress>(
            "SELECT * FROM user_addresses WHERE user_id = ? LIMIT 1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(address)
    }

    /// Update the user's phone number.
    pub async fn update_phone(
        pool: &MySqlPool,
        user_id: i64,
        phone: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE users SET phone_number = ?, updated_at = NOW() WHERE id = ?",
        )
        .bind(phone)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update the user's preferred locker.
    pub async fn update_locker(
        pool: &MySqlPool,
        user_id: i64,
        data: UpdateLockerData,
    ) -> Result<(), ApiError> {
        sqlx::query(
            r#"
            UPDATE users SET
                default_inpost_locker = COALESCE(?, default_inpost_locker),
                locker_id = COALESCE(?, locker_id),
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(&data.default_inpost_locker)
        .bind(data.locker_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Export all user data for GDPR compliance.
    pub async fn export_data(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<GdprExport, ApiError> {
        let user: serde_json::Value = {
            let row = sqlx::query_as::<_, UserSafe>(
                r#"
                SELECT
                    id, email, username, first_name, last_name, description,
                    phone_number, avatar_id, language, locker_id,
                    stripe_customer_id, stripe_connect_account_id, stripe_connect_status,
                    stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
                    stripe_connect_onboarded_at,
                    privacy_policy_accepted, terms_of_service_accepted,
                    marketing_emails_accepted, consents_accepted_at,
                    google_id, facebook_id, social_provider, social_provider_id,
                    average_rating, review_count,
                    activation_code_expires_at, last_unread_notification_at,
                    preferred_item_types, default_inpost_locker,
                    email_verified_at, created_at, updated_at
                FROM users WHERE id = ?
                "#,
            )
            .bind(user_id)
            .fetch_one(pool)
            .await?;
            serde_json::to_value(row)?
        };

        let address: Option<serde_json::Value> = {
            let row = sqlx::query_as::<_, UserAddress>(
                "SELECT * FROM user_addresses WHERE user_id = ? LIMIT 1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            match row {
                Some(a) => Some(serde_json::to_value(a)?),
                None => None,
            }
        };

        let books: Vec<serde_json::Value> = {
            let rows = sqlx::query("SELECT * FROM books WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(pool)
                .await?;
            rows.iter()
                .map(|_| serde_json::json!({"note": "book_data"}))
                .collect()
        };

        let trades: Vec<serde_json::Value> = {
            let rows = sqlx::query(
                "SELECT * FROM trades WHERE initiator_id = ? OR recipient_id = ?",
            )
            .bind(user_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
            rows.iter()
                .map(|_| serde_json::json!({"note": "trade_data"}))
                .collect()
        };

        let messages: Vec<serde_json::Value> = {
            let rows = sqlx::query(
                r#"
                SELECT m.* FROM messages m
                INNER JOIN trades t ON t.id = m.trade_id
                WHERE t.initiator_id = ? OR t.recipient_id = ?
                "#,
            )
            .bind(user_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
            rows.iter()
                .map(|_| serde_json::json!({"note": "message_data"}))
                .collect()
        };

        let swipes: Vec<serde_json::Value> = {
            let rows = sqlx::query("SELECT * FROM swipes WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(pool)
                .await?;
            rows.iter()
                .map(|_| serde_json::json!({"note": "swipe_data"}))
                .collect()
        };

        let reviews: Vec<serde_json::Value> = {
            let rows = sqlx::query(
                "SELECT * FROM reviews WHERE reviewer_id = ? OR reviewed_user_id = ?",
            )
            .bind(user_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
            rows.iter()
                .map(|_| serde_json::json!({"note": "review_data"}))
                .collect()
        };

        let payment_methods: Vec<serde_json::Value> = {
            let rows = sqlx::query(
                "SELECT * FROM stripe_payment_methods WHERE user_id = ?",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?;
            rows.iter()
                .map(|_| serde_json::json!({"note": "payment_method_data"}))
                .collect()
        };

        Ok(GdprExport {
            user,
            address,
            books,
            trades,
            messages,
            swipes,
            reviews,
            payment_methods,
            exported_at: Utc::now().to_rfc3339(),
        })
    }

    /// GDPR account deletion -- anonymize personal data and soft-delete.
    pub async fn delete_account(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<(), ApiError> {
        let mut tx = pool.begin().await?;
        let now = Utc::now().naive_utc();
        let anon_email = format!("deleted_{}@anonymized.swapie.app", user_id);
        let anon_username = format!("deleted_{}", user_id);

        // Anonymize user record
        sqlx::query(
            r#"
            UPDATE users SET
                email = ?,
                username = ?,
                password = 'DELETED',
                first_name = 'Deleted',
                last_name = 'User',
                description = NULL,
                phone_number = NULL,
                avatar_id = NULL,
                google_id = NULL,
                facebook_id = NULL,
                social_provider = NULL,
                social_provider_id = NULL,
                default_inpost_locker = NULL,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&anon_email)
        .bind(&anon_username)
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        // Delete address
        sqlx::query("DELETE FROM user_addresses WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        // Soft-delete all books
        sqlx::query(
            "UPDATE books SET deleted_at = ?, status = 'inactive', updated_at = ? WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        // Remove device tokens
        sqlx::query("DELETE FROM device_tokens WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        // Remove payment methods
        sqlx::query("DELETE FROM stripe_payment_methods WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Get GDPR consent status for a user.
    pub async fn get_gdpr_consent(
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<GdprConsent, ApiError> {
        let row: (bool, bool, bool, Option<chrono::NaiveDateTime>) = sqlx::query_as(
            r#"
            SELECT privacy_policy_accepted, terms_of_service_accepted,
                   marketing_emails_accepted, consents_accepted_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        Ok(GdprConsent {
            privacy_policy_accepted: row.0,
            terms_of_service_accepted: row.1,
            marketing_emails_accepted: row.2,
            consents_accepted_at: row.3,
        })
    }

    /// Update GDPR consent flags.
    pub async fn update_gdpr_consent(
        pool: &MySqlPool,
        user_id: i64,
        data: UpdateGdprConsentData,
    ) -> Result<GdprConsent, ApiError> {
        let now = Utc::now().naive_utc();

        sqlx::query(
            r#"
            UPDATE users SET
                privacy_policy_accepted = COALESCE(?, privacy_policy_accepted),
                terms_of_service_accepted = COALESCE(?, terms_of_service_accepted),
                marketing_emails_accepted = COALESCE(?, marketing_emails_accepted),
                consents_accepted_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(data.privacy_policy_accepted)
        .bind(data.terms_of_service_accepted)
        .bind(data.marketing_emails_accepted)
        .bind(now)
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await?;

        Self::get_gdpr_consent(pool, user_id).await
    }

    /// Block another user.
    pub async fn block_user(
        pool: &MySqlPool,
        blocker_id: i64,
        blocked_id: i64,
        reason: Option<&str>,
    ) -> Result<(), ApiError> {
        if blocker_id == blocked_id {
            return Err(ApiError::bad_request("users.cannot_block_self"));
        }

        // Check user exists
        let _user: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE id = ?")
            .bind(blocked_id)
            .fetch_one(pool)
            .await?;

        // Check if already blocked
        let (existing,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_blocks WHERE blocker_id = ? AND blocked_id = ?",
        )
        .bind(blocker_id)
        .bind(blocked_id)
        .fetch_one(pool)
        .await?;

        if existing > 0 {
            return Err(ApiError::conflict("users.already_blocked"));
        }

        let now = chrono::Utc::now().naive_utc();

        sqlx::query(
            "INSERT INTO user_blocks (blocker_id, blocked_id, reason, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(blocker_id)
        .bind(blocked_id)
        .bind(reason)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Unblock a user.
    pub async fn unblock_user(
        pool: &MySqlPool,
        blocker_id: i64,
        blocked_id: i64,
    ) -> Result<(), ApiError> {
        let result = sqlx::query(
            "DELETE FROM user_blocks WHERE blocker_id = ? AND blocked_id = ?",
        )
        .bind(blocker_id)
        .bind(blocked_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::not_found("users.not_blocked"));
        }

        Ok(())
    }

    /// Get list of blocked users with their info.
    pub async fn get_blocked_users(
        pool: &MySqlPool,
        blocker_id: i64,
    ) -> Result<Vec<BlockedUserInfo>, ApiError> {
        let blocks = sqlx::query_as::<_, crate::models::UserBlock>(
            "SELECT * FROM user_blocks WHERE blocker_id = ? ORDER BY created_at DESC",
        )
        .bind(blocker_id)
        .fetch_all(pool)
        .await?;

        let mut blocked_users: Vec<BlockedUserInfo> = Vec::new();
        for block in blocks {
            let user: Option<(i64, String, String, Option<i64>)> = sqlx::query_as(
                "SELECT id, username, first_name, avatar_id FROM users WHERE id = ?",
            )
            .bind(block.blocked_id)
            .fetch_optional(pool)
            .await?;

            if let Some(u) = user {
                blocked_users.push(BlockedUserInfo {
                    block_id: block.id,
                    user_id: u.0,
                    username: u.1,
                    first_name: u.2,
                    avatar_id: u.3,
                    reason: block.reason,
                    blocked_at: block.created_at,
                });
            }
        }

        Ok(blocked_users)
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    async fn get_stats(pool: &MySqlPool, user_id: i64) -> Result<UserStats, ApiError> {
        let books_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM books WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let active_books: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM books WHERE user_id = ? AND deleted_at IS NULL AND status = 'active'",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let completed_trades: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE (initiator_id = ? OR recipient_id = ?) AND status = 'completed'",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let active_trades: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM trades WHERE (initiator_id = ? OR recipient_id = ?) AND status IN ('pending', 'accepted', 'countered', 'awaiting_shipment', 'shipped')",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let total_swipes: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM swipes WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let matches_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM book_matches WHERE book_owner_id = ? OR interested_user_id = ?",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(UserStats {
            books_count: books_count.0,
            active_books_count: active_books.0,
            trades_completed: completed_trades.0,
            trades_active: active_trades.0,
            total_swipes: total_swipes.0,
            matches_count: matches_count.0,
        })
    }
}
