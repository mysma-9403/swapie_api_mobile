use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::dto::{ApiResponse, PaginatedResponse, PaginationParams};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::{Post, PostMeta};

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
pub struct CreatePostRequest {
    #[validate(length(min = 1, max = 255, message = "validation.title_length"))]
    pub title: String,
    pub slug: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub meta: Option<Vec<PostMetaEntry>>,
}

#[derive(Debug, Deserialize)]
pub struct PostMetaEntry {
    pub meta_key: String,
    pub meta_value: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePostRequest {
    #[validate(length(min = 1, max = 255, message = "validation.title_length"))]
    pub title: Option<String>,
    pub slug: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub meta: Option<Vec<PostMetaEntry>>,
}

#[derive(Debug, Deserialize)]
pub struct BulkDeletePostsRequest {
    pub ids: Vec<i64>,
}

// ── Path extractors ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PostTypePath {
    pub post_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PostTypeIdPath {
    pub post_type: String,
    pub id: i64,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PostWithMeta {
    #[serde(flatten)]
    pub post: Post,
    pub meta: Vec<PostMeta>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/posts/{postType?}
#[tracing::instrument(skip(pool, auth, query))]
pub async fn list_posts(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<Option<String>>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let post_type = path.into_inner();

    let (total, posts) = if let Some(ref pt) = post_type {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM posts WHERE post_type = ?",
        )
        .bind(pt)
        .fetch_one(pool.get_ref())
        .await? as u64;

        let posts = sqlx::query_as::<_, Post>(
            "SELECT * FROM posts WHERE post_type = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(pt)
        .bind(query.per_page())
        .bind(query.offset())
        .fetch_all(pool.get_ref())
        .await?;

        (total, posts)
    } else {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM posts")
            .fetch_one(pool.get_ref())
            .await? as u64;

        let posts = sqlx::query_as::<_, Post>(
            "SELECT * FROM posts ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(query.per_page())
        .bind(query.offset())
        .fetch_all(pool.get_ref())
        .await?;

        (total, posts)
    };

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(posts, query.page(), query.per_page(), total)))
}

/// POST /api/v1/posts/{postType}
#[tracing::instrument(skip(pool, auth, body))]
pub async fn create_post(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<CreatePostRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let post_type = path.into_inner();

    body.validate()
        .map_err(|_| ApiError::validation("validation.invalid_input"))?;

    let slug = body.slug.clone().unwrap_or_else(|| {
        body.title
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    });

    let status = body.status.as_deref().unwrap_or("draft");
    let now = chrono::Utc::now().naive_utc();

    let result = sqlx::query(
        "INSERT INTO posts (post_type, title, slug, content, status, author_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&post_type)
    .bind(&body.title)
    .bind(&slug)
    .bind(&body.content)
    .bind(status)
    .bind(auth.user_id)
    .bind(now)
    .bind(now)
    .execute(pool.get_ref())
    .await?;

    let post_id = result.last_insert_id() as i64;

    // Insert meta if provided
    if let Some(ref meta) = body.meta {
        for entry in meta {
            sqlx::query(
                "INSERT INTO post_meta (post_id, meta_key, meta_value) VALUES (?, ?, ?)",
            )
            .bind(post_id)
            .bind(&entry.meta_key)
            .bind(&entry.meta_value)
            .execute(pool.get_ref())
            .await?;
        }
    }

    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(pool.get_ref())
        .await?;

    let meta = sqlx::query_as::<_, PostMeta>(
        "SELECT * FROM post_meta WHERE post_id = ?",
    )
    .bind(post_id)
    .fetch_all(pool.get_ref())
    .await?;

    let data = PostWithMeta { post, meta };
    Ok(HttpResponse::Created().json(ApiResponse::success(data, i18n::t(&lang, "posts.created"))))
}

/// GET /api/v1/posts/{postType}/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_post(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<(String, i64)>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let (post_type, post_id) = path.into_inner();

    let post = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE id = ? AND post_type = ?",
    )
    .bind(post_id)
    .bind(&post_type)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("posts.not_found"))?;

    let meta = sqlx::query_as::<_, PostMeta>(
        "SELECT * FROM post_meta WHERE post_id = ?",
    )
    .bind(post_id)
    .fetch_all(pool.get_ref())
    .await?;

    let data = PostWithMeta { post, meta };
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// PUT /api/v1/posts/{postType}/{id}
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_post(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<(String, i64)>,
    body: web::Json<UpdatePostRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let (post_type, post_id) = path.into_inner();

    let _post = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE id = ? AND post_type = ?",
    )
    .bind(post_id)
    .bind(&post_type)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("posts.not_found"))?;

    let mut set_clauses: Vec<String> = Vec::new();

    if let Some(ref title) = body.title {
        set_clauses.push(format!("title = '{}'", title));
    }
    if let Some(ref slug) = body.slug {
        set_clauses.push(format!("slug = '{}'", slug));
    }
    if let Some(ref content) = body.content {
        set_clauses.push(format!("content = '{}'", content));
    }
    if let Some(ref status) = body.status {
        set_clauses.push(format!("status = '{}'", status));
    }

    if !set_clauses.is_empty() {
        set_clauses.push("updated_at = NOW()".to_string());
        let sql = format!("UPDATE posts SET {} WHERE id = ?", set_clauses.join(", "));
        sqlx::query(&sql)
            .bind(post_id)
            .execute(pool.get_ref())
            .await?;
    }

    // Update meta if provided
    if let Some(ref meta) = body.meta {
        sqlx::query("DELETE FROM post_meta WHERE post_id = ?")
            .bind(post_id)
            .execute(pool.get_ref())
            .await?;

        for entry in meta {
            sqlx::query(
                "INSERT INTO post_meta (post_id, meta_key, meta_value) VALUES (?, ?, ?)",
            )
            .bind(post_id)
            .bind(&entry.meta_key)
            .bind(&entry.meta_value)
            .execute(pool.get_ref())
            .await?;
        }
    }

    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(pool.get_ref())
        .await?;

    let meta = sqlx::query_as::<_, PostMeta>(
        "SELECT * FROM post_meta WHERE post_id = ?",
    )
    .bind(post_id)
    .fetch_all(pool.get_ref())
    .await?;

    let data = PostWithMeta { post, meta };
    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "posts.updated"))))
}

/// DELETE /api/v1/posts/{postType}/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn delete_post(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<(String, i64)>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let (post_type, post_id) = path.into_inner();

    // Delete meta first
    sqlx::query("DELETE FROM post_meta WHERE post_id = ?")
        .bind(post_id)
        .execute(pool.get_ref())
        .await?;

    let result = sqlx::query("DELETE FROM posts WHERE id = ? AND post_type = ?")
        .bind(post_id)
        .bind(&post_type)
        .execute(pool.get_ref())
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("posts.not_found"));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "posts.deleted"))))
}

/// GET /api/v1/posts (no postType filter)
#[tracing::instrument(skip(pool, auth, query))]
pub async fn list_posts_default(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM posts")
        .fetch_one(pool.get_ref())
        .await? as u64;

    let posts = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(query.per_page())
    .bind(query.offset())
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(posts, query.page(), query.per_page(), total)))
}

/// POST /api/v1/posts/{postType}/bulk-delete
#[tracing::instrument(skip(pool, auth, body))]
pub async fn bulk_delete_posts(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
    body: web::Json<BulkDeletePostsRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let post_type = path.into_inner();

    if body.ids.is_empty() {
        return Err(ApiError::bad_request("general.no_ids_provided"));
    }

    let placeholders = body.ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Delete meta
    let meta_sql = format!("DELETE FROM post_meta WHERE post_id IN ({})", placeholders);
    let mut meta_query = sqlx::query(&meta_sql);
    for id in &body.ids {
        meta_query = meta_query.bind(id);
    }
    meta_query.execute(pool.get_ref()).await?;

    // Delete posts
    let sql = format!(
        "DELETE FROM posts WHERE id IN ({}) AND post_type = ?",
        placeholders
    );
    let mut query = sqlx::query(&sql);
    for id in &body.ids {
        query = query.bind(id);
    }
    let result = query.bind(&post_type).execute(pool.get_ref()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(
        serde_json::json!({"deleted": result.rows_affected()}),
        i18n::t(&lang, "posts.bulk_deleted"),
    )))
}
