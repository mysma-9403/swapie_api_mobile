use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::config::SharedConfig;
use crate::dto::{PaginatedResponse, PaginationMeta, PaginationParams};
use crate::errors::ApiError;
use crate::models::{Book, BookChange, BookImage, Genre, Tag};

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateBookData {
    pub book_type: String,
    pub title: String,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub description: Option<String>,
    pub condition: String,
    pub for_exchange: bool,
    pub for_sale: bool,
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
    pub genre_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
    pub image_paths: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBookData {
    pub title: Option<String>,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub description: Option<String>,
    pub condition: Option<String>,
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
    pub genre_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
    pub image_paths: Option<Vec<String>>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BookFilters {
    pub book_type: Option<String>,
    pub condition: Option<String>,
    pub status: Option<String>,
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,
    pub location: Option<String>,
    pub for_exchange: Option<bool>,
    pub for_sale: Option<bool>,
    pub search: Option<String>,
    pub genre_id: Option<i64>,
    pub tag_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct BookDetail {
    #[serde(flatten)]
    pub book: Book,
    pub images: Vec<BookImage>,
    pub genres: Vec<Genre>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExternalBookData {
    pub title: Option<String>,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub description: Option<String>,
    pub pages_count: Option<i32>,
    pub cover_url: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub publish_year: Option<i32>,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct BookService;

impl BookService {
    /// Create a new book with images, genres, and tags.
    pub async fn create_book(
        pool: &MySqlPool,
        user_id: i64,
        data: CreateBookData,
    ) -> Result<BookDetail, ApiError> {
        let mut tx = pool.begin().await?;

        let now = Utc::now().naive_utc();
        let use_profile_filters = data.use_profile_filters.unwrap_or(false);

        let result = sqlx::query(
            r#"
            INSERT INTO books (
                user_id, `type`, title, author, isbn, description,
                `condition`, status, for_exchange, for_sale, price,
                location, latitude, longitude, category, language,
                pages_count, book_format, min_players, max_players,
                playing_time, age_rating, wanted_isbn, wanted_title,
                use_profile_filters, views_count, likes_count,
                created_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?,
                ?, 'active', ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, 0, 0,
                ?, ?
            )
            "#,
        )
        .bind(user_id)
        .bind(&data.book_type)
        .bind(&data.title)
        .bind(&data.author)
        .bind(&data.isbn)
        .bind(&data.description)
        .bind(&data.condition)
        .bind(data.for_exchange)
        .bind(data.for_sale)
        .bind(&data.price)
        .bind(&data.location)
        .bind(&data.latitude)
        .bind(&data.longitude)
        .bind(&data.category)
        .bind(&data.language)
        .bind(data.pages_count)
        .bind(&data.book_format)
        .bind(data.min_players)
        .bind(data.max_players)
        .bind(data.playing_time)
        .bind(data.age_rating)
        .bind(&data.wanted_isbn)
        .bind(&data.wanted_title)
        .bind(use_profile_filters)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let book_id = result.last_insert_id() as i64;

        // Insert images
        if let Some(ref paths) = data.image_paths {
            for (i, path) in paths.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO book_images (book_id, image_path, is_primary, `order`) VALUES (?, ?, ?, ?)",
                )
                .bind(book_id)
                .bind(path)
                .bind(i == 0)
                .bind(i as i32)
                .execute(&mut *tx)
                .await?;
            }
        }

        // Insert genre relations
        if let Some(ref genre_ids) = data.genre_ids {
            for genre_id in genre_ids {
                sqlx::query("INSERT INTO book_genre (book_id, genre_id) VALUES (?, ?)")
                    .bind(book_id)
                    .bind(genre_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Insert tag relations
        if let Some(ref tag_ids) = data.tag_ids {
            for tag_id in tag_ids {
                sqlx::query("INSERT INTO book_tag (book_id, tag_id) VALUES (?, ?)")
                    .bind(book_id)
                    .bind(tag_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        tx.commit().await?;

        Self::get_book(pool, book_id).await
    }

    /// Update a book, logging each changed field to book_changes.
    pub async fn update_book(
        pool: &MySqlPool,
        book_id: i64,
        user_id: i64,
        data: UpdateBookData,
    ) -> Result<BookDetail, ApiError> {
        let existing = sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(book_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("book.not_found"))?;

        if existing.user_id != user_id {
            return Err(ApiError::forbidden());
        }

        let mut tx = pool.begin().await?;
        let now = Utc::now().naive_utc();
        let ip = data.ip_address.clone();
        let ua = data.user_agent.clone();

        // Helper macro for logging changes
        macro_rules! log_change {
            ($tx:expr, $field:expr, $old:expr, $new:expr) => {
                sqlx::query(
                    r#"
                    INSERT INTO book_changes
                        (book_id, user_id, field_name, old_value, new_value, ip_address, user_agent, created_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(book_id)
                .bind(user_id)
                .bind($field)
                .bind($old)
                .bind($new)
                .bind(&ip)
                .bind(&ua)
                .bind(now)
                .execute(&mut *$tx)
                .await?;
            };
        }

        // Track and apply field changes
        if let Some(ref title) = data.title {
            if *title != existing.title {
                log_change!(tx, "title", Some(existing.title.clone()), Some(title.clone()));
            }
        }
        if let Some(ref author) = data.author {
            let old = existing.author.clone().unwrap_or_default();
            if *author != old {
                log_change!(tx, "author", existing.author.clone(), Some(author.clone()));
            }
        }
        if let Some(ref description) = data.description {
            let old = existing.description.clone().unwrap_or_default();
            if *description != old {
                log_change!(tx, "description", existing.description.clone(), Some(description.clone()));
            }
        }
        if let Some(ref condition) = data.condition {
            let old = existing.condition.to_string();
            if *condition != old {
                log_change!(tx, "condition", Some(old), Some(condition.clone()));
            }
        }
        if let Some(ref price) = data.price {
            let old = existing.price.map(|p| p.to_string());
            let new_val = price.to_string();
            if old.as_deref() != Some(&new_val) {
                log_change!(tx, "price", old, Some(new_val));
            }
        }

        // Apply the update
        sqlx::query(
            r#"
            UPDATE books SET
                title = COALESCE(?, title),
                author = COALESCE(?, author),
                isbn = COALESCE(?, isbn),
                description = COALESCE(?, description),
                `condition` = COALESCE(?, `condition`),
                for_exchange = COALESCE(?, for_exchange),
                for_sale = COALESCE(?, for_sale),
                price = COALESCE(?, price),
                location = COALESCE(?, location),
                latitude = COALESCE(?, latitude),
                longitude = COALESCE(?, longitude),
                category = COALESCE(?, category),
                language = COALESCE(?, language),
                pages_count = COALESCE(?, pages_count),
                book_format = COALESCE(?, book_format),
                min_players = COALESCE(?, min_players),
                max_players = COALESCE(?, max_players),
                playing_time = COALESCE(?, playing_time),
                age_rating = COALESCE(?, age_rating),
                wanted_isbn = COALESCE(?, wanted_isbn),
                wanted_title = COALESCE(?, wanted_title),
                use_profile_filters = COALESCE(?, use_profile_filters),
                updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&data.title)
        .bind(&data.author)
        .bind(&data.isbn)
        .bind(&data.description)
        .bind(&data.condition)
        .bind(data.for_exchange)
        .bind(data.for_sale)
        .bind(&data.price)
        .bind(&data.location)
        .bind(&data.latitude)
        .bind(&data.longitude)
        .bind(&data.category)
        .bind(&data.language)
        .bind(data.pages_count)
        .bind(&data.book_format)
        .bind(data.min_players)
        .bind(data.max_players)
        .bind(data.playing_time)
        .bind(data.age_rating)
        .bind(&data.wanted_isbn)
        .bind(&data.wanted_title)
        .bind(data.use_profile_filters)
        .bind(now)
        .bind(book_id)
        .execute(&mut *tx)
        .await?;

        // Replace genre relations if provided
        if let Some(ref genre_ids) = data.genre_ids {
            sqlx::query("DELETE FROM book_genre WHERE book_id = ?")
                .bind(book_id)
                .execute(&mut *tx)
                .await?;
            for genre_id in genre_ids {
                sqlx::query("INSERT INTO book_genre (book_id, genre_id) VALUES (?, ?)")
                    .bind(book_id)
                    .bind(genre_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Replace tag relations if provided
        if let Some(ref tag_ids) = data.tag_ids {
            sqlx::query("DELETE FROM book_tag WHERE book_id = ?")
                .bind(book_id)
                .execute(&mut *tx)
                .await?;
            for tag_id in tag_ids {
                sqlx::query("INSERT INTO book_tag (book_id, tag_id) VALUES (?, ?)")
                    .bind(book_id)
                    .bind(tag_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Replace images if provided
        if let Some(ref paths) = data.image_paths {
            sqlx::query("DELETE FROM book_images WHERE book_id = ?")
                .bind(book_id)
                .execute(&mut *tx)
                .await?;
            for (i, path) in paths.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO book_images (book_id, image_path, is_primary, `order`) VALUES (?, ?, ?, ?)",
                )
                .bind(book_id)
                .bind(path)
                .bind(i == 0)
                .bind(i as i32)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        Self::get_book(pool, book_id).await
    }

    /// Soft delete a book by setting deleted_at.
    pub async fn delete_book(
        pool: &MySqlPool,
        book_id: i64,
        user_id: i64,
    ) -> Result<(), ApiError> {
        let result = sqlx::query(
            "UPDATE books SET deleted_at = NOW(), status = 'inactive', updated_at = NOW() WHERE id = ? AND user_id = ? AND deleted_at IS NULL",
        )
        .bind(book_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::not_found("book.not_found"));
        }

        Ok(())
    }

    /// Fetch a single book with its images, genres, and tags (eager loaded).
    pub async fn get_book(pool: &MySqlPool, book_id: i64) -> Result<BookDetail, ApiError> {
        let book = sqlx::query_as::<_, Book>(
            "SELECT * FROM books WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(book_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("book.not_found"))?;

        let images = sqlx::query_as::<_, BookImage>(
            "SELECT * FROM book_images WHERE book_id = ? ORDER BY `order` ASC",
        )
        .bind(book_id)
        .fetch_all(pool)
        .await?;

        let genres = sqlx::query_as::<_, Genre>(
            r#"
            SELECT g.* FROM genres g
            INNER JOIN book_genre bg ON bg.genre_id = g.id
            WHERE bg.book_id = ?
            "#,
        )
        .bind(book_id)
        .fetch_all(pool)
        .await?;

        let tags = sqlx::query_as::<_, Tag>(
            r#"
            SELECT t.* FROM tags t
            INNER JOIN book_tag bt ON bt.tag_id = t.id
            WHERE bt.book_id = ?
            "#,
        )
        .bind(book_id)
        .fetch_all(pool)
        .await?;

        Ok(BookDetail {
            book,
            images,
            genres,
            tags,
        })
    }

    /// List books belonging to a specific user, paginated.
    pub async fn list_user_books(
        pool: &MySqlPool,
        user_id: i64,
        pagination: &PaginationParams,
    ) -> Result<PaginatedResponse<Book>, ApiError> {
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM books WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let books = sqlx::query_as::<_, Book>(
            r#"
            SELECT * FROM books
            WHERE user_id = ? AND deleted_at IS NULL
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
            books,
            pagination.page(),
            pagination.per_page(),
            total.0 as u64,
        ))
    }

    /// List books with filtering support, paginated.
    pub async fn list_books(
        pool: &MySqlPool,
        filters: &BookFilters,
        pagination: &PaginationParams,
    ) -> Result<PaginatedResponse<Book>, ApiError> {
        // Build dynamic WHERE clauses
        let mut conditions = vec!["b.deleted_at IS NULL".to_string()];
        let mut count_conditions = vec!["deleted_at IS NULL".to_string()];

        if let Some(ref bt) = filters.book_type {
            conditions.push(format!("b.`type` = '{}'", bt));
            count_conditions.push(format!("`type` = '{}'", bt));
        }
        if let Some(ref cond) = filters.condition {
            conditions.push(format!("b.`condition` = '{}'", cond));
            count_conditions.push(format!("`condition` = '{}'", cond));
        }
        if let Some(ref status) = filters.status {
            conditions.push(format!("b.status = '{}'", status));
            count_conditions.push(format!("status = '{}'", status));
        } else {
            conditions.push("b.status = 'active'".to_string());
            count_conditions.push("status = 'active'".to_string());
        }
        if let Some(min) = filters.min_price {
            conditions.push(format!("b.price >= {}", min));
            count_conditions.push(format!("price >= {}", min));
        }
        if let Some(max) = filters.max_price {
            conditions.push(format!("b.price <= {}", max));
            count_conditions.push(format!("price <= {}", max));
        }
        if let Some(ref loc) = filters.location {
            conditions.push(format!("b.location LIKE '%{}%'", loc));
            count_conditions.push(format!("location LIKE '%{}%'", loc));
        }
        if let Some(fe) = filters.for_exchange {
            conditions.push(format!("b.for_exchange = {}", fe as i32));
            count_conditions.push(format!("for_exchange = {}", fe as i32));
        }
        if let Some(fs) = filters.for_sale {
            conditions.push(format!("b.for_sale = {}", fs as i32));
            count_conditions.push(format!("for_sale = {}", fs as i32));
        }
        if let Some(ref search) = filters.search {
            let like = format!(
                "(b.title LIKE '%{}%' OR b.author LIKE '%{}%' OR b.isbn LIKE '%{}%')",
                search, search, search
            );
            conditions.push(like.clone());
            let count_like = format!(
                "(title LIKE '%{s}%' OR author LIKE '%{s}%' OR isbn LIKE '%{s}%')",
                s = search
            );
            count_conditions.push(count_like);
        }

        let mut join_clause = String::new();
        if let Some(gid) = filters.genre_id {
            join_clause.push_str(&format!(
                " INNER JOIN book_genre bg ON bg.book_id = b.id AND bg.genre_id = {}",
                gid
            ));
        }
        if let Some(tid) = filters.tag_id {
            join_clause.push_str(&format!(
                " INNER JOIN book_tag bt ON bt.book_id = b.id AND bt.tag_id = {}",
                tid
            ));
        }

        let where_clause = conditions.join(" AND ");
        let count_where = count_conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM books WHERE {}", count_where);
        let total: (i64,) = sqlx::query_as(&count_sql).fetch_one(pool).await?;

        let query_sql = format!(
            r#"
            SELECT b.* FROM books b
            {}
            WHERE {}
            ORDER BY b.created_at DESC
            LIMIT {} OFFSET {}
            "#,
            join_clause,
            where_clause,
            pagination.per_page(),
            pagination.offset()
        );

        let books = sqlx::query_as::<_, Book>(&query_sql)
            .fetch_all(pool)
            .await?;

        Ok(PaginatedResponse::new(
            books,
            pagination.page(),
            pagination.per_page(),
            total.0 as u64,
        ))
    }

    /// Get similar books based on matching genres and tags.
    pub async fn get_similar_books(
        pool: &MySqlPool,
        book_id: i64,
        limit: i32,
    ) -> Result<Vec<Book>, ApiError> {
        let books = sqlx::query_as::<_, Book>(
            r#"
            SELECT DISTINCT b.* FROM books b
            LEFT JOIN book_genre bg ON bg.book_id = b.id
            LEFT JOIN book_tag bt ON bt.book_id = b.id
            WHERE b.id != ?
              AND b.deleted_at IS NULL
              AND b.status = 'active'
              AND (
                  bg.genre_id IN (SELECT genre_id FROM book_genre WHERE book_id = ?)
                  OR bt.tag_id IN (SELECT tag_id FROM book_tag WHERE book_id = ?)
              )
            ORDER BY b.likes_count DESC, b.views_count DESC
            LIMIT ?
            "#,
        )
        .bind(book_id)
        .bind(book_id)
        .bind(book_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(books)
    }

    /// Get the change log for a book.
    pub async fn get_book_changes(
        pool: &MySqlPool,
        book_id: i64,
    ) -> Result<Vec<BookChange>, ApiError> {
        let changes = sqlx::query_as::<_, BookChange>(
            "SELECT * FROM book_changes WHERE book_id = ? ORDER BY created_at DESC",
        )
        .bind(book_id)
        .fetch_all(pool)
        .await?;

        Ok(changes)
    }

    /// Fetch external book/board game data from Lagano Library API.
    ///
    /// Endpoint: `GET https://library.lagano.pl/isbn/{isbn}`
    /// No API key required (same-server service).
    pub async fn fetch_external_book(
        config: &SharedConfig,
        isbn_or_ean: &str,
    ) -> Result<ExternalBookData, ApiError> {
        let url = format!("{}/isbn/{}", config.book_api_url, isbn_or_ean);

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::not_found("book.external_not_found"));
        }

        if !resp.status().is_success() {
            return Err(ApiError::external_service("book.external_fetch_failed"));
        }

        let entry: serde_json::Value = resp.json().await?;

        let title = entry
            .get("title")
            .and_then(|v| v.as_str())
            .map(String::from);

        let author = entry
            .get("author")
            .or_else(|| entry.get("authors"))
            .and_then(|v| {
                if v.is_string() {
                    v.as_str().map(String::from)
                } else if v.is_array() {
                    v.as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|a| {
                            a.as_str()
                                .map(String::from)
                                .or_else(|| a.get("name").and_then(|n| n.as_str()).map(String::from))
                        })
                } else {
                    None
                }
            });

        let description = entry
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let pages_count = entry
            .get("pages_count")
            .or_else(|| entry.get("number_of_pages"))
            .or_else(|| entry.get("pages"))
            .and_then(|v| v.as_i64())
            .map(|n| n as i32);

        let cover_url = entry
            .get("cover_url")
            .or_else(|| entry.get("cover"))
            .or_else(|| entry.get("image"))
            .and_then(|v| {
                if v.is_string() {
                    v.as_str().map(String::from)
                } else if v.is_object() {
                    v.get("large")
                        .or_else(|| v.get("medium"))
                        .and_then(|u| u.as_str())
                        .map(String::from)
                } else {
                    None
                }
            });

        let language = entry
            .get("language")
            .and_then(|v| v.as_str())
            .map(String::from);

        let publisher = entry
            .get("publisher")
            .or_else(|| entry.get("publishers"))
            .and_then(|v| {
                if v.is_string() {
                    v.as_str().map(String::from)
                } else if v.is_array() {
                    v.as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|p| {
                            p.as_str()
                                .map(String::from)
                                .or_else(|| p.get("name").and_then(|n| n.as_str()).map(String::from))
                        })
                } else {
                    None
                }
            });

        let publish_year = entry
            .get("publish_year")
            .or_else(|| entry.get("year"))
            .or_else(|| entry.get("publish_date"))
            .and_then(|v| {
                if v.is_number() {
                    v.as_i64().map(|n| n as i32)
                } else if v.is_string() {
                    v.as_str().and_then(|s| {
                        s.split_whitespace()
                            .find_map(|w| w.parse::<i32>().ok().filter(|y| *y > 1000 && *y < 3000))
                    })
                } else {
                    None
                }
            });

        Ok(ExternalBookData {
            title,
            author,
            isbn: Some(isbn_or_ean.to_string()),
            description,
            pages_count,
            cover_url,
            language,
            publisher,
            publish_year,
        })
    }

    /// Increment the view counter for a book.
    pub async fn increment_views(pool: &MySqlPool, book_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE books SET views_count = views_count + 1, updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(book_id)
        .execute(pool)
        .await?;

        Ok(())
    }
}
