use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::models::{Genre, Tag};

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

// ── Query DTOs ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GenreFilter {
    #[serde(rename = "type")]
    pub genre_type: Option<String>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TranslationsResponse {
    pub translations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct RegulationResponse {
    pub content: String,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/translations/{lang}
#[tracing::instrument]
pub async fn get_translations(
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = path.into_inner();

    // Load translations from embedded locale files
    let translations_json = match lang.as_str() {
        "en" => include_str!("../../locales/en.json"),
        "pl" => include_str!("../../locales/pl.json"),
        _ => include_str!("../../locales/en.json"),
    };

    let translations: std::collections::HashMap<String, String> =
        serde_json::from_str(translations_json)
            .map_err(|_| ApiError::internal())?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(
        TranslationsResponse { translations },
        "Translations loaded",
    )))
}

/// GET /api/v1/genres
#[tracing::instrument(skip(pool, query))]
pub async fn list_genres(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    query: web::Query<GenreFilter>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let genres = if let Some(ref genre_type) = query.genre_type {
        sqlx::query_as::<_, Genre>(
            "SELECT * FROM genres WHERE type = ? ORDER BY name ASC",
        )
        .bind(genre_type)
        .fetch_all(pool.get_ref())
        .await?
    } else {
        sqlx::query_as::<_, Genre>("SELECT * FROM genres ORDER BY name ASC")
            .fetch_all(pool.get_ref())
            .await?
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(genres, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/tags
#[tracing::instrument(skip(pool))]
pub async fn list_tags(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let tags = sqlx::query_as::<_, Tag>("SELECT * FROM tags ORDER BY name ASC")
        .fetch_all(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(tags, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/regulations
#[tracing::instrument(skip(pool))]
pub async fn get_regulations(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let setting = sqlx::query_as::<_, (Option<String>, chrono::NaiveDateTime)>(
        "SELECT option_value, updated_at FROM settings WHERE option_name = 'regulations' LIMIT 1",
    )
    .bind("regulations")
    .fetch_optional(pool.get_ref())
    .await?;

    let data = if let Some((content, updated_at)) = setting {
        RegulationResponse {
            content: content.unwrap_or_default(),
            updated_at: Some(updated_at),
        }
    } else {
        RegulationResponse {
            content: String::new(),
            updated_at: None,
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/privacy-policy
#[tracing::instrument(skip(pool))]
pub async fn get_privacy_policy(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let setting = sqlx::query_as::<_, (Option<String>, chrono::NaiveDateTime)>(
        "SELECT option_value, updated_at FROM settings WHERE option_name = 'privacy_policy' LIMIT 1",
    )
    .bind("privacy_policy")
    .fetch_optional(pool.get_ref())
    .await?;

    let data = if let Some((content, updated_at)) = setting {
        RegulationResponse {
            content: content.unwrap_or_default(),
            updated_at: Some(updated_at),
        }
    } else {
        RegulationResponse {
            content: String::new(),
            updated_at: None,
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}
