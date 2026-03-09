use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::ApiResponse;
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::{Taxonomy, Term};

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

// ── Request DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTermRequest {
    #[validate(length(min = 1, max = 255, message = "validation.name_length"))]
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTermRequest {
    #[validate(length(min = 1, max = 255, message = "validation.name_length"))]
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct BulkDeleteTermsRequest {
    pub ids: Vec<i64>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/terms/{taxonomy}
#[tracing::instrument(skip(pool, auth))]
pub async fn list_terms(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let taxonomy_slug = path.into_inner();

    let taxonomy = sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = ?",
    )
    .bind(&taxonomy_slug)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.taxonomy_not_found"))?;

    let terms = sqlx::query_as::<_, Term>(
        "SELECT * FROM terms WHERE taxonomy_id = ? ORDER BY sort_order ASC, name ASC",
    )
    .bind(taxonomy.id)
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(terms, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/terms/{taxonomy}
#[tracing::instrument(skip(pool, auth, body))]
pub async fn create_term(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<CreateTermRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let taxonomy_slug = path.into_inner();

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let taxonomy = sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = ?",
    )
    .bind(&taxonomy_slug)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.taxonomy_not_found"))?;

    let slug = body.slug.clone().unwrap_or_else(|| {
        body.name
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    });

    let sort_order = body.sort_order.unwrap_or(0);
    let now = chrono::Utc::now().naive_utc();

    let result = sqlx::query(
        "INSERT INTO terms (taxonomy_id, name, slug, description, parent_id, sort_order,
         created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(taxonomy.id)
    .bind(&body.name)
    .bind(&slug)
    .bind(&body.description)
    .bind(body.parent_id)
    .bind(sort_order)
    .bind(now)
    .bind(now)
    .execute(pool.get_ref())
    .await?;

    let term_id = result.last_insert_id() as i64;

    let term = sqlx::query_as::<_, Term>("SELECT * FROM terms WHERE id = ?")
        .bind(term_id)
        .fetch_one(pool.get_ref())
        .await?;

    Ok(HttpResponse::Created().json(ApiResponse::success(term, i18n::t(&lang, "terms.created"))))
}

/// GET /api/v1/terms/{taxonomy}/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_term(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<(String, i64)>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let (taxonomy_slug, term_id) = path.into_inner();

    let taxonomy = sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = ?",
    )
    .bind(&taxonomy_slug)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.taxonomy_not_found"))?;

    let term = sqlx::query_as::<_, Term>(
        "SELECT * FROM terms WHERE id = ? AND taxonomy_id = ?",
    )
    .bind(term_id)
    .bind(taxonomy.id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.not_found"))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(term, i18n::t(&lang, "general.success"))))
}

/// PUT /api/v1/terms/{taxonomy}/{id}
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_term(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<(String, i64)>,
    body: web::Json<UpdateTermRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let (taxonomy_slug, term_id) = path.into_inner();

    let taxonomy = sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = ?",
    )
    .bind(&taxonomy_slug)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.taxonomy_not_found"))?;

    let _term = sqlx::query_as::<_, Term>(
        "SELECT * FROM terms WHERE id = ? AND taxonomy_id = ?",
    )
    .bind(term_id)
    .bind(taxonomy.id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.not_found"))?;

    let mut set_clauses: Vec<String> = Vec::new();

    if let Some(ref name) = body.name {
        set_clauses.push(format!("name = '{}'", name));
    }
    if let Some(ref slug) = body.slug {
        set_clauses.push(format!("slug = '{}'", slug));
    }
    if let Some(ref description) = body.description {
        set_clauses.push(format!("description = '{}'", description));
    }
    if let Some(parent_id) = body.parent_id {
        set_clauses.push(format!("parent_id = {}", parent_id));
    }
    if let Some(sort_order) = body.sort_order {
        set_clauses.push(format!("sort_order = {}", sort_order));
    }

    if !set_clauses.is_empty() {
        set_clauses.push("updated_at = NOW()".to_string());
        let sql = format!("UPDATE terms SET {} WHERE id = ?", set_clauses.join(", "));
        sqlx::query(&sql)
            .bind(term_id)
            .execute(pool.get_ref())
            .await?;
    }

    let updated_term = sqlx::query_as::<_, Term>("SELECT * FROM terms WHERE id = ?")
        .bind(term_id)
        .fetch_one(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(updated_term, i18n::t(&lang, "terms.updated"))))
}

/// DELETE /api/v1/terms/{taxonomy}/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn delete_term(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<(String, i64)>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let (taxonomy_slug, term_id) = path.into_inner();

    let taxonomy = sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = ?",
    )
    .bind(&taxonomy_slug)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.taxonomy_not_found"))?;

    let result = sqlx::query("DELETE FROM terms WHERE id = ? AND taxonomy_id = ?")
        .bind(term_id)
        .bind(taxonomy.id)
        .execute(pool.get_ref())
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("terms.not_found"));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "terms.deleted"))))
}

/// POST /api/v1/terms/{taxonomy}/bulk-delete
#[tracing::instrument(skip(pool, auth, body))]
pub async fn bulk_delete_terms(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<BulkDeleteTermsRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let taxonomy_slug = path.into_inner();

    if body.ids.is_empty() {
        return Err(ApiError::bad_request("general.no_ids_provided"));
    }

    let taxonomy = sqlx::query_as::<_, Taxonomy>(
        "SELECT * FROM taxonomies WHERE slug = ?",
    )
    .bind(&taxonomy_slug)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("terms.taxonomy_not_found"))?;

    let placeholders = body.ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "DELETE FROM terms WHERE id IN ({}) AND taxonomy_id = ?",
        placeholders
    );
    let mut query = sqlx::query(&sql);
    for id in &body.ids {
        query = query.bind(id);
    }
    let result = query.bind(taxonomy.id).execute(pool.get_ref()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(
        serde_json::json!({"deleted": result.rows_affected()}),
        i18n::t(&lang, "terms.bulk_deleted"),
    )))
}
