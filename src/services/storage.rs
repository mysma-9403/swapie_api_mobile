use chrono::NaiveDateTime;
use hmac::{Hmac, Mac};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::MySqlPool;

use crate::config::Config;
use crate::errors::ApiError;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BookImage {
    pub id: i64,
    pub book_id: i64,
    pub url: String,
    pub storage_key: String,
    pub content_type: String,
    pub sort_order: i32,
    pub created_at: NaiveDateTime,
}

// ── Storage Service ─────────────────────────────────────────────────────

/// S3-compatible file storage service (DigitalOcean Spaces, AWS S3, MinIO, etc.).
pub struct StorageService {
    client: reqwest::Client,
    endpoint: String,
    bucket: String,
    region: String,
    access_key: String,
    secret_key: String,
    public_url: String,
}

impl StorageService {
    /// Create a new `StorageService` from the application config.
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: config.s3_endpoint.clone(),
            bucket: config.s3_bucket.clone(),
            region: config.s3_region.clone(),
            access_key: config.s3_access_key.clone(),
            secret_key: config.s3_secret_key.clone(),
            public_url: config.s3_url.clone(),
        }
    }

    /// Upload a file to the S3-compatible storage.
    ///
    /// Returns the public URL of the uploaded file.
    pub async fn upload_file(
        &self,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String, ApiError> {
        let now = chrono::Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        let url = format!("{}/{}/{}", self.endpoint, bucket, key);

        let payload_hash = Self::sha256_hex(&data);

        // Build canonical request for AWS Signature V4.
        let canonical_uri = format!("/{}/{}", bucket, key);
        let host = self
            .endpoint
            .replace("https://", "")
            .replace("http://", "");

        let canonical_headers = format!(
            "content-type:{}\nhost:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            content_type, host, payload_hash, amz_date
        );
        let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "PUT\n{}\n\n{}\n{}\n{}",
            canonical_uri, canonical_headers, signed_headers, payload_hash
        );

        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            Self::sha256_hex(canonical_request.as_bytes())
        );

        // Derive the signing key.
        let k_date = Self::hmac_sha256(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date_stamp.as_bytes(),
        );
        let k_region = Self::hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = Self::hmac_sha256(&k_region, b"s3");
        let k_signing = Self::hmac_sha256(&k_service, b"aws4_request");

        let signature = hex::encode(Self::hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key, credential_scope, signed_headers, signature
        );

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_str(content_type).unwrap());
        headers.insert(
            "x-amz-content-sha256",
            HeaderValue::from_str(&payload_hash).unwrap(),
        );
        headers.insert("x-amz-date", HeaderValue::from_str(&amz_date).unwrap());
        headers.insert(
            "x-amz-acl",
            HeaderValue::from_static("public-read"),
        );
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&authorization).unwrap(),
        );

        let response = self
            .client
            .put(&url)
            .headers(headers)
            .body(data)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("S3 upload failed: {:?}", e);
                ApiError::external_service("storage.upload_failed")
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("S3 upload error ({}): {}", status, body);
            return Err(ApiError::external_service("storage.upload_failed"));
        }

        Ok(self.get_url(bucket, key))
    }

    /// Delete a file from S3-compatible storage.
    pub async fn delete_file(&self, bucket: &str, key: &str) -> Result<(), ApiError> {
        let now = chrono::Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        let url = format!("{}/{}/{}", self.endpoint, bucket, key);
        let host = self
            .endpoint
            .replace("https://", "")
            .replace("http://", "");

        let payload_hash = Self::sha256_hex(b"");

        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            host, payload_hash, amz_date
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "DELETE\n/{}/{}\n\n{}\n{}\n{}",
            bucket, key, canonical_headers, signed_headers, payload_hash
        );

        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            Self::sha256_hex(canonical_request.as_bytes())
        );

        let k_date = Self::hmac_sha256(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date_stamp.as_bytes(),
        );
        let k_region = Self::hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = Self::hmac_sha256(&k_region, b"s3");
        let k_signing = Self::hmac_sha256(&k_service, b"aws4_request");

        let signature = hex::encode(Self::hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key, credential_scope, signed_headers, signature
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-amz-content-sha256",
            HeaderValue::from_str(&payload_hash).unwrap(),
        );
        headers.insert("x-amz-date", HeaderValue::from_str(&amz_date).unwrap());
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&authorization).unwrap(),
        );

        let response = self
            .client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("S3 delete failed: {:?}", e);
                ApiError::external_service("storage.delete_failed")
            })?;

        if !response.status().is_success() && response.status().as_u16() != 404 {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("S3 delete error ({}): {}", status, body);
            return Err(ApiError::external_service("storage.delete_failed"));
        }

        Ok(())
    }

    /// Get the public URL for a stored object.
    pub fn get_url(&self, bucket: &str, key: &str) -> String {
        if self.public_url.is_empty() {
            format!("{}/{}/{}", self.endpoint, bucket, key)
        } else {
            format!("{}/{}", self.public_url, key)
        }
    }

    /// Upload a book image and create the corresponding database record.
    pub async fn upload_book_image(
        &self,
        pool: &MySqlPool,
        book_id: i64,
        file_data: Vec<u8>,
        content_type: &str,
        order: i32,
    ) -> Result<BookImage, ApiError> {
        let extension = match content_type {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            _ => "jpg",
        };

        let key = format!(
            "books/{}/{}_{}.{}",
            book_id,
            uuid::Uuid::new_v4(),
            order,
            extension
        );

        let url = self
            .upload_file(&self.bucket, &key, file_data, content_type)
            .await?;

        let result = sqlx::query(
            r#"
            INSERT INTO book_images (book_id, url, storage_key, content_type, sort_order, created_at)
            VALUES (?, ?, ?, ?, ?, NOW())
            "#,
        )
        .bind(book_id)
        .bind(&url)
        .bind(&key)
        .bind(content_type)
        .bind(order)
        .execute(pool)
        .await?;

        let image_id = result.last_insert_id() as i64;

        let image: BookImage = sqlx::query_as("SELECT * FROM book_images WHERE id = ?")
            .bind(image_id)
            .fetch_one(pool)
            .await?;

        Ok(image)
    }

    /// Delete a book image from storage and the database.
    pub async fn delete_book_image(
        &self,
        pool: &MySqlPool,
        image_id: i64,
    ) -> Result<(), ApiError> {
        let image: BookImage = sqlx::query_as("SELECT * FROM book_images WHERE id = ?")
            .bind(image_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::not_found("storage.image_not_found"))?;

        // Delete from S3.
        self.delete_file(&self.bucket, &image.storage_key).await?;

        // Delete from database.
        sqlx::query("DELETE FROM book_images WHERE id = ?")
            .bind(image_id)
            .execute(pool)
            .await?;

        Ok(())
    }

    // ── Private helpers ─────────────────────────────────────────────────

    fn sha256_hex(data: &[u8]) -> String {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }
}
