use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use validator::Validate;

use crate::config::SharedConfig;
use crate::dto::{ApiResponse, PaginationParams};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::BookService;
use crate::services::book::{BookFilters as ServiceBookFilters, CreateBookData, UpdateBookData};

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

// ── Request/Response DTOs ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct BookFilters {
    #[serde(rename = "type")]
    pub book_type: Option<String>,
    pub condition: Option<String>,
    pub status: Option<String>,
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl BookFilters {
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }
    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }
    pub fn offset(&self) -> u64 {
        ((self.page() - 1) as u64) * (self.per_page() as u64)
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBookRequest {
    pub title: Option<String>,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub description: Option<String>,
    pub condition: Option<String>,
    pub status: Option<String>,
    pub for_exchange: Option<bool>,
    pub for_sale: Option<bool>,
    pub price: Option<Decimal>,
    pub location: Option<String>,
    pub latitude: Option<Decimal>,
    pub longitude: Option<Decimal>,
    pub category: Option<String>,
    pub language: Option<String>,
    pub pages_count: Option<i32>,
    pub book_format: Option<String>,
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
    pub playing_time: Option<i32>,
    pub age_rating: Option<i32>,
    pub wanted_isbn: Option<String>,
    pub wanted_title: Option<String>,
    pub use_profile_filters: Option<bool>,
    pub genres: Option<Vec<i64>>,
    pub tags: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct SimilarBooksQuery {
    pub book_id: i64,
    pub limit: Option<u32>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/books - Create a book with multipart images
#[tracing::instrument(skip(pool, config, auth, payload))]
pub async fn create_book(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    mut payload: Multipart,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    // Parse multipart fields
    let mut title: Option<String> = None;
    let mut author: Option<String> = None;
    let mut isbn: Option<String> = None;
    let mut description: Option<String> = None;
    let mut condition: Option<String> = None;
    let mut book_type: Option<String> = None;
    let mut for_exchange = false;
    let mut for_sale = false;
    let mut price: Option<Decimal> = None;
    let mut location: Option<String> = None;
    let mut category: Option<String> = None;
    let mut book_language: Option<String> = None;
    let mut image_data: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|_| ApiError::bad_request("general.invalid_multipart"))?;
        let content_disposition = field.content_disposition()
            .ok_or_else(|| ApiError::bad_request("general.invalid_multipart"))?
            .clone();
        let field_name = content_disposition
            .get_name()
            .unwrap_or("")
            .to_string();

        if field_name.starts_with("images") {
            let filename = content_disposition
                .get_filename()
                .unwrap_or("upload.jpg")
                .to_string();
            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|_| ApiError::bad_request("general.upload_error"))?;
                bytes.extend_from_slice(&data);
            }
            image_data.push((filename, bytes));
        } else {
            let mut value = String::new();
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|_| ApiError::bad_request("general.upload_error"))?;
                value.push_str(&String::from_utf8_lossy(&data));
            }
            match field_name.as_str() {
                "title" => title = Some(value),
                "author" => author = Some(value),
                "isbn" => isbn = Some(value),
                "description" => description = Some(value),
                "condition" => condition = Some(value),
                "type" => book_type = Some(value),
                "for_exchange" => for_exchange = value == "true" || value == "1",
                "for_sale" => for_sale = value == "true" || value == "1",
                "price" => price = value.parse().ok(),
                "location" => location = Some(value),
                "category" => category = Some(value),
                "language" => book_language = Some(value),
                _ => {}
            }
        }
    }

    let title = title.ok_or_else(|| ApiError::validation("validation.required"))?;
    let condition = condition.unwrap_or_else(|| "used_good".to_string());
    let book_type = book_type.unwrap_or_else(|| "book".to_string());

    // Upload images to S3 and build image paths
    let s3_client = reqwest::Client::new();
    let mut image_paths: Vec<String> = Vec::new();
    for (idx, (filename, bytes)) in image_data.iter().enumerate() {
        let key = format!("books/new/{}-{}", idx, filename);
        let upload_url = format!("{}/{}/{}", config.s3_endpoint, config.s3_bucket, key);
        let content_type = if filename.ends_with(".png") { "image/png" } else { "image/jpeg" };
        let _ = s3_client
            .put(&upload_url)
            .header("Content-Type", content_type)
            .header("x-amz-acl", "public-read")
            .header("Authorization", format!("AWS {}:{}", config.s3_access_key, config.s3_secret_key))
            .body(bytes.clone())
            .send()
            .await;
        image_paths.push(format!("{}/{}", config.s3_url, key));
    }

    let data = CreateBookData {
        book_type,
        title,
        author,
        isbn,
        description,
        condition,
        for_exchange,
        for_sale,
        price,
        location,
        latitude: None,
        longitude: None,
        category,
        language: book_language,
        pages_count: None,
        book_format: None,
        min_players: None,
        max_players: None,
        playing_time: None,
        age_rating: None,
        wanted_isbn: None,
        wanted_title: None,
        use_profile_filters: None,
        genre_ids: None,
        tag_ids: None,
        image_paths: if image_paths.is_empty() { None } else { Some(image_paths) },
    };

    let book_detail = BookService::create_book(pool.get_ref(), auth.user_id, data).await?;
    Ok(HttpResponse::Created().json(ApiResponse::success(book_detail, i18n::t(&lang, "books.created"))))
}

/// GET /api/v1/books
#[tracing::instrument(skip(pool, auth, query))]
pub async fn list_books(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<BookFilters>,
) -> Result<HttpResponse, ApiError> {
    let _lang = lang_from_req(&req);

    let filters = ServiceBookFilters {
        book_type: query.book_type.clone(),
        condition: query.condition.clone(),
        status: query.status.clone(),
        min_price: query.min_price,
        max_price: query.max_price,
        search: query.search.clone(),
        location: None,
        for_exchange: None,
        for_sale: None,
        genre_id: None,
        tag_id: None,
    };

    let pagination = PaginationParams {
        page: Some(query.page()),
        per_page: Some(query.per_page()),
    };

    let result = BookService::list_books(pool.get_ref(), &filters, &pagination).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// GET /api/v1/books/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_book(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    BookService::increment_views(pool.get_ref(), book_id).await?;
    let book_detail = BookService::get_book(pool.get_ref(), book_id).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(book_detail, i18n::t(&lang, "general.success"))))
}

/// PUT /api/v1/books/{id}
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_book(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    body: web::Json<UpdateBookRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    let data = UpdateBookData {
        title: body.title.clone(),
        author: body.author.clone(),
        isbn: body.isbn.clone(),
        description: body.description.clone(),
        condition: body.condition.clone(),
        for_exchange: body.for_exchange,
        for_sale: body.for_sale,
        price: body.price,
        location: body.location.clone(),
        latitude: body.latitude,
        longitude: body.longitude,
        category: body.category.clone(),
        language: body.language.clone(),
        pages_count: body.pages_count,
        book_format: body.book_format.clone(),
        min_players: body.min_players,
        max_players: body.max_players,
        playing_time: body.playing_time,
        age_rating: body.age_rating,
        wanted_isbn: body.wanted_isbn.clone(),
        wanted_title: body.wanted_title.clone(),
        use_profile_filters: body.use_profile_filters,
        genre_ids: body.genres.clone(),
        tag_ids: body.tags.clone(),
        image_paths: None,
        ip_address: None,
        user_agent: None,
    };

    let updated = BookService::update_book(pool.get_ref(), book_id, auth.user_id, data).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(updated, i18n::t(&lang, "books.updated"))))
}

/// POST /api/v1/books/{id}/update - Multipart update with images
#[tracing::instrument(skip(pool, config, auth, payload))]
pub async fn update_book_multipart(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
    mut payload: Multipart,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    // Parse multipart fields
    let mut updates: Vec<(String, String)> = Vec::new();
    let mut image_data: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|_| ApiError::bad_request("general.invalid_multipart"))?;
        let content_disposition = field.content_disposition()
            .ok_or_else(|| ApiError::bad_request("general.invalid_multipart"))?
            .clone();
        let field_name = content_disposition
            .get_name()
            .unwrap_or("")
            .to_string();

        if field_name.starts_with("images") {
            let filename = content_disposition
                .get_filename()
                .unwrap_or("upload.jpg")
                .to_string();
            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|_| ApiError::bad_request("general.upload_error"))?;
                bytes.extend_from_slice(&data);
            }
            image_data.push((filename, bytes));
        } else {
            let mut value = String::new();
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|_| ApiError::bad_request("general.upload_error"))?;
                value.push_str(&String::from_utf8_lossy(&data));
            }
            updates.push((field_name, value));
        }
    }

    // Convert multipart fields to UpdateBookData
    let mut data = UpdateBookData {
        title: None, author: None, isbn: None, description: None,
        condition: None, for_exchange: None, for_sale: None, price: None,
        location: None, latitude: None, longitude: None, category: None,
        language: None, pages_count: None, book_format: None,
        min_players: None, max_players: None, playing_time: None,
        age_rating: None, wanted_isbn: None, wanted_title: None,
        use_profile_filters: None, genre_ids: None, tag_ids: None,
        image_paths: None, ip_address: None, user_agent: None,
    };

    for (field_name, value) in &updates {
        match field_name.as_str() {
            "title" => data.title = Some(value.clone()),
            "author" => data.author = Some(value.clone()),
            "isbn" => data.isbn = Some(value.clone()),
            "description" => data.description = Some(value.clone()),
            "condition" => data.condition = Some(value.clone()),
            "location" => data.location = Some(value.clone()),
            "category" => data.category = Some(value.clone()),
            "language" => data.language = Some(value.clone()),
            "for_exchange" => data.for_exchange = Some(value == "true" || value == "1"),
            "for_sale" => data.for_sale = Some(value == "true" || value == "1"),
            "price" => data.price = value.parse().ok(),
            _ => {}
        }
    }

    // Upload images to S3 and build image paths
    if !image_data.is_empty() {
        let s3_client = reqwest::Client::new();
        let mut paths: Vec<String> = Vec::new();
        for (idx, (filename, bytes)) in image_data.iter().enumerate() {
            let key = format!("books/{}/{}-{}", book_id, idx, filename);
            let upload_url = format!("{}/{}/{}", config.s3_endpoint, config.s3_bucket, key);
            let content_type = if filename.ends_with(".png") { "image/png" } else { "image/jpeg" };
            let _ = s3_client
                .put(&upload_url)
                .header("Content-Type", content_type)
                .header("x-amz-acl", "public-read")
                .header("Authorization", format!("AWS {}:{}", config.s3_access_key, config.s3_secret_key))
                .body(bytes.clone())
                .send()
                .await;
            paths.push(format!("{}/{}", config.s3_url, key));
        }
        data.image_paths = Some(paths);
    }

    let updated = BookService::update_book(pool.get_ref(), book_id, auth.user_id, data).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(updated, i18n::t(&lang, "books.updated"))))
}

/// DELETE /api/v1/books/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn delete_book(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    BookService::delete_book(pool.get_ref(), book_id, auth.user_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "books.deleted"))))
}

/// GET /api/v1/books/user
#[tracing::instrument(skip(pool, auth, query))]
pub async fn get_user_books(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let _lang = lang_from_req(&req);

    let result = BookService::list_user_books(pool.get_ref(), auth.user_id, &query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// GET /api/v1/books/similar
#[tracing::instrument(skip(pool, auth, query))]
pub async fn get_similar_books(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<SimilarBooksQuery>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let limit = query.limit.unwrap_or(10).min(50) as i32;

    let similar = BookService::get_similar_books(pool.get_ref(), query.book_id, limit).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(similar, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/books/{id}/changes
#[tracing::instrument(skip(pool, auth))]
pub async fn get_book_changes(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let book_id = path.into_inner();

    // Verify ownership before showing changes
    let book_detail = BookService::get_book(pool.get_ref(), book_id).await?;
    if book_detail.book.user_id != auth.user_id {
        return Err(ApiError::forbidden());
    }

    let changes = BookService::get_book_changes(pool.get_ref(), book_id).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(changes, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/books/external/{eanOrIsbn}
#[tracing::instrument(skip(pool, config, auth))]
pub async fn get_external_book(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    config: web::Data<SharedConfig>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let ean_or_isbn = path.into_inner();

    let info = BookService::fetch_external_book(&config, &ean_or_isbn).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(info, i18n::t(&lang, "general.success"))))
}
