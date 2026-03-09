use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;
use sqlx::MySqlPool;

use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::{Book, BookMatch, Trade};
use crate::services::SwapCenterService;

// ── Helper ───────────────────────────────────────────────────────────────────

fn lang_from_req(req: &HttpRequest) -> String {
    req.headers()
        .get("Accept-Language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("en")
        .split(',')
        .next()
        .unwrap_or("en")
        .split('-')
        .next()
        .unwrap_or("en")
        .to_string()
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SwapCenterOverview {
    pub active_swaps: i64,
    pub pending_offers: i64,
    pub you_like_count: i64,
    pub others_like_count: i64,
    pub matches_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SwapSummary {
    pub trade: Trade,
    pub other_user: SwapUserInfo,
}

#[derive(Debug, Serialize)]
pub struct SwapUserInfo {
    pub id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub avatar_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct LikedBookEntry {
    pub book: Book,
    pub owner: SwapUserInfo,
}

#[derive(Debug, Serialize)]
pub struct SwapDetails {
    pub user: SwapUserInfo,
    pub my_liked_books: Vec<Book>,
    pub their_liked_books: Vec<Book>,
    pub matches: Vec<BookMatch>,
}

#[derive(Debug, Serialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub activity_type: String,
    pub description: String,
    pub trade_id: Option<i64>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct MatchEntry {
    pub match_info: BookMatch,
    pub other_user: SwapUserInfo,
    pub book: Book,
}

#[derive(Debug, Serialize)]
pub struct MatchDetails {
    pub user: SwapUserInfo,
    pub matches: Vec<BookMatch>,
    pub my_books: Vec<Book>,
    pub their_books: Vec<Book>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/swap-center
#[tracing::instrument(skip(pool, auth))]
pub async fn get_swap_center(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let overview = SwapCenterService::get_overview(pool.get_ref(), auth.user_id).await?;

    let data = SwapCenterOverview {
        active_swaps: overview.active_swaps,
        pending_offers: overview.pending_offers,
        you_like_count: overview.you_like_count,
        others_like_count: overview.others_like_count,
        matches_count: overview.matches_count,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/swap-center/swaps
#[tracing::instrument(skip(pool, auth))]
pub async fn get_swaps(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let trades = SwapCenterService::get_active_trades(pool.get_ref(), auth.user_id).await?;

    let mut swaps: Vec<SwapSummary> = Vec::new();
    for trade in trades {
        let other_user_id = if trade.initiator_id == auth.user_id {
            trade.recipient_id
        } else {
            trade.initiator_id
        };

        if let Some(u) = SwapCenterService::get_user_info(pool.get_ref(), other_user_id).await? {
            swaps.push(SwapSummary {
                trade,
                other_user: SwapUserInfo {
                    id: u.0,
                    username: u.1,
                    first_name: u.2,
                    last_name: u.3,
                    avatar_id: u.4,
                },
            });
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(swaps, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/swap-center/you-like
#[tracing::instrument(skip(pool, auth))]
pub async fn get_you_like(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let books = SwapCenterService::get_liked_books(pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(books, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/swap-center/others-like
#[tracing::instrument(skip(pool, auth))]
pub async fn get_others_like(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let books = SwapCenterService::get_books_others_liked(pool.get_ref(), auth.user_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(books, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/swap-center/swap-details/{userId}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_swap_details(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let other_user_id = path.into_inner();

    let details = SwapCenterService::get_swap_details(
        pool.get_ref(),
        auth.user_id,
        other_user_id,
    )
    .await?;

    let user_info = SwapCenterService::get_user_info(pool.get_ref(), other_user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("users.not_found"))?;

    let data = SwapDetails {
        user: SwapUserInfo {
            id: user_info.0,
            username: user_info.1,
            first_name: user_info.2,
            last_name: user_info.3,
            avatar_id: user_info.4,
        },
        my_liked_books: details.their_books,
        their_liked_books: details.your_books,
        matches: details.matches,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/swap-center/activity
#[tracing::instrument(skip(pool, auth))]
pub async fn get_activity(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let service_activities = SwapCenterService::get_activity(pool.get_ref(), auth.user_id).await?;

    let activities: Vec<ActivityEntry> = service_activities
        .into_iter()
        .map(|a| ActivityEntry {
            id: a.id,
            activity_type: a.activity_type.clone(),
            description: a.title.unwrap_or_else(|| i18n::t(&lang, &format!("activity.{}", a.activity_type))),
            trade_id: if a.activity_type.starts_with("trade_") {
                Some(a.related_id)
            } else {
                None
            },
            created_at: a.created_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(activities, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/matches
#[tracing::instrument(skip(pool, auth))]
pub async fn get_matches(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let matches = SwapCenterService::get_matches(pool.get_ref(), auth.user_id).await?;

    let mut entries: Vec<MatchEntry> = Vec::new();
    for m in matches {
        let other_user_id = if m.book_owner_id == auth.user_id {
            m.interested_user_id
        } else {
            m.book_owner_id
        };

        let user = SwapCenterService::get_user_info(pool.get_ref(), other_user_id).await?;

        let book = sqlx::query_as::<_, Book>("SELECT * FROM books WHERE id = ?")
            .bind(m.owner_book_id)
            .fetch_optional(pool.get_ref())
            .await?;

        if let (Some(u), Some(b)) = (user, book) {
            entries.push(MatchEntry {
                match_info: m,
                other_user: SwapUserInfo {
                    id: u.0,
                    username: u.1,
                    first_name: u.2,
                    last_name: u.3,
                    avatar_id: u.4,
                },
                book: b,
            });
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(entries, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/matches/{user_id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_match_details(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let other_user_id = path.into_inner();

    let user_info = SwapCenterService::get_user_info(pool.get_ref(), other_user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("users.not_found"))?;

    let matches = SwapCenterService::get_match_details(
        pool.get_ref(),
        auth.user_id,
        other_user_id,
    )
    .await?;

    let my_books = SwapCenterService::get_user_active_books(pool.get_ref(), auth.user_id).await?;
    let their_books = SwapCenterService::get_user_active_books(pool.get_ref(), other_user_id).await?;

    let data = MatchDetails {
        user: SwapUserInfo {
            id: user_info.0,
            username: user_info.1,
            first_name: user_info.2,
            last_name: user_info.3,
            avatar_id: user_info.4,
        },
        matches,
        my_books,
        their_books,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}
