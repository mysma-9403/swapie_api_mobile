use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};

use crate::config::SharedConfig;
use crate::errors::ApiError;

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locker {
    pub name: String,
    pub address: String,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub provider: String,
    pub status: Option<String>,
    pub opening_hours: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeliveryOption {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub provider: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub estimated_days: Option<i32>,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct FeeConfig {
    pub protection_fee: Decimal,
    pub platform_fee_percent: Decimal,
    pub min_platform_fee: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct ShipmentAddress {
    pub name: String,
    pub street: Option<String>,
    pub city: String,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub locker_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShipmentLabel {
    pub shipment_id: String,
    pub label_url: String,
    pub tracking_number: Option<String>,
    pub provider: String,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct DeliveryService;

impl DeliveryService {
    /// Search InPost lockers by query string and optional city filter.
    pub async fn search_inpost_lockers(
        config: &SharedConfig,
        query: &str,
        city: Option<&str>,
    ) -> Result<Vec<Locker>, ApiError> {
        let mut url = format!(
            "{}/v1/points?name={}&type=parcel_locker&per_page=20",
            config.inpost_api_base_url, query
        );
        if let Some(c) = city {
            url.push_str(&format!("&city={}", c));
        }

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.inpost_api_token))
            .send()
            .await?;

        if !resp.status().is_success() {
            tracing::error!("InPost API error: {}", resp.status());
            return Err(ApiError::external_service("delivery.inpost_api_error"));
        }

        let body: serde_json::Value = resp.json().await?;
        let lockers = Self::parse_inpost_response(&body);

        Ok(lockers)
    }

    /// Validate that a specific InPost locker exists and get its details.
    pub async fn validate_inpost_locker(
        config: &SharedConfig,
        locker_name: &str,
    ) -> Result<Locker, ApiError> {
        let url = format!(
            "{}/v1/points/{}",
            config.inpost_api_base_url, locker_name
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.inpost_api_token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ApiError::not_found("delivery.locker_not_found"));
        }

        let body: serde_json::Value = resp.json().await?;
        Self::parse_inpost_point(&body)
            .ok_or_else(|| ApiError::not_found("delivery.locker_not_found"))
    }

    /// Get nearest InPost lockers by geographic coordinates.
    pub async fn get_nearest_inpost(
        config: &SharedConfig,
        lat: f64,
        lng: f64,
        limit: i32,
    ) -> Result<Vec<Locker>, ApiError> {
        let url = format!(
            "{}/v1/points?relative_point={},{}&type=parcel_locker&per_page={}&sort_by=distance_to_relative_point",
            config.inpost_api_base_url, lat, lng, limit
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.inpost_api_token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ApiError::external_service("delivery.inpost_api_error"));
        }

        let body: serde_json::Value = resp.json().await?;
        Ok(Self::parse_inpost_response(&body))
    }

    /// Get a single InPost locker by name.
    pub async fn get_inpost_locker(
        config: &SharedConfig,
        locker_name: &str,
    ) -> Result<Locker, ApiError> {
        Self::validate_inpost_locker(config, locker_name).await
    }

    /// Search Orlen lockers by query string.
    pub async fn search_orlen_lockers(
        config: &SharedConfig,
        query: &str,
    ) -> Result<Vec<Locker>, ApiError> {
        if config.orlen_api_base_url.is_empty() {
            return Err(ApiError::external_service("delivery.orlen_not_configured"));
        }

        let url = format!(
            "{}/points?search={}&limit=20",
            config.orlen_api_base_url, query
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.orlen_api_token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ApiError::external_service("delivery.orlen_api_error"));
        }

        let body: serde_json::Value = resp.json().await?;
        let items = body["items"]
            .as_array()
            .or_else(|| body["data"].as_array())
            .cloned()
            .unwrap_or_default();

        let lockers: Vec<Locker> = items
            .iter()
            .filter_map(|item| {
                Some(Locker {
                    name: item["name"].as_str()?.to_string(),
                    address: item["address"].as_str().unwrap_or_default().to_string(),
                    city: item["city"].as_str().map(String::from),
                    postal_code: item["postal_code"].as_str().map(String::from),
                    latitude: item["latitude"].as_f64().unwrap_or(0.0),
                    longitude: item["longitude"].as_f64().unwrap_or(0.0),
                    provider: "orlen".to_string(),
                    status: item["status"].as_str().map(String::from),
                    opening_hours: item["opening_hours"].as_str().map(String::from),
                })
            })
            .collect();

        Ok(lockers)
    }

    /// Get nearest Orlen lockers by geographic coordinates.
    pub async fn get_nearest_orlen(
        config: &SharedConfig,
        lat: f64,
        lng: f64,
        limit: i32,
    ) -> Result<Vec<Locker>, ApiError> {
        if config.orlen_api_base_url.is_empty() {
            return Err(ApiError::external_service("delivery.orlen_not_configured"));
        }

        let url = format!(
            "{}/points?latitude={}&longitude={}&limit={}&sort_by=distance",
            config.orlen_api_base_url, lat, lng, limit
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.orlen_api_token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ApiError::external_service("delivery.orlen_api_error"));
        }

        let body: serde_json::Value = resp.json().await?;
        let items = body["items"]
            .as_array()
            .or_else(|| body["data"].as_array())
            .cloned()
            .unwrap_or_default();

        let lockers: Vec<Locker> = items
            .iter()
            .filter_map(|item| {
                Some(Locker {
                    name: item["name"].as_str()?.to_string(),
                    address: item["address"].as_str().unwrap_or_default().to_string(),
                    city: item["city"].as_str().map(String::from),
                    postal_code: item["postal_code"].as_str().map(String::from),
                    latitude: item["latitude"].as_f64().unwrap_or(0.0),
                    longitude: item["longitude"].as_f64().unwrap_or(0.0),
                    provider: "orlen".to_string(),
                    status: item["status"].as_str().map(String::from),
                    opening_hours: item["opening_hours"].as_str().map(String::from),
                })
            })
            .collect();

        Ok(lockers)
    }

    /// Get a single Orlen locker by name.
    pub async fn get_orlen_locker(
        config: &SharedConfig,
        locker_name: &str,
    ) -> Result<Locker, ApiError> {
        if config.orlen_api_base_url.is_empty() {
            return Err(ApiError::external_service("delivery.orlen_not_configured"));
        }

        let url = format!(
            "{}/points/{}",
            config.orlen_api_base_url, locker_name
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.orlen_api_token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ApiError::not_found("delivery.locker_not_found"));
        }

        let body: serde_json::Value = resp.json().await?;

        Ok(Locker {
            name: body["name"]
                .as_str()
                .unwrap_or(locker_name)
                .to_string(),
            address: body["address"].as_str().unwrap_or_default().to_string(),
            city: body["city"].as_str().map(String::from),
            postal_code: body["postal_code"].as_str().map(String::from),
            latitude: body["latitude"].as_f64().unwrap_or(0.0),
            longitude: body["longitude"].as_f64().unwrap_or(0.0),
            provider: "orlen".to_string(),
            status: body["status"].as_str().map(String::from),
            opening_hours: body["opening_hours"].as_str().map(String::from),
        })
    }

    /// Get all active delivery options from the database.
    pub async fn get_delivery_options(
        pool: &MySqlPool,
    ) -> Result<Vec<DeliveryOption>, ApiError> {
        let options = sqlx::query_as::<_, DeliveryOption>(
            "SELECT * FROM delivery_options WHERE is_active = 1 ORDER BY price ASC",
        )
        .fetch_all(pool)
        .await?;

        Ok(options)
    }

    /// Get the current fee configuration from the settings table.
    pub async fn get_fees(pool: &MySqlPool) -> Result<FeeConfig, ApiError> {
        let protection_fee: Decimal = Self::get_setting(pool, "protection_fee")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| Decimal::new(299, 2));

        let platform_fee_percent: Decimal = Self::get_setting(pool, "platform_fee_percent")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| Decimal::new(5, 0));

        let min_platform_fee: Decimal = Self::get_setting(pool, "min_platform_fee")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| Decimal::new(199, 2));

        Ok(FeeConfig {
            protection_fee,
            platform_fee_percent,
            min_platform_fee,
        })
    }

    /// Generate a shipment label via the appropriate delivery provider API.
    pub async fn generate_shipment_label(
        config: &SharedConfig,
        provider: &str,
        from: &ShipmentAddress,
        to: &ShipmentAddress,
        size: &str,
    ) -> Result<ShipmentLabel, ApiError> {
        match provider {
            "inpost" => Self::generate_inpost_label(config, from, to, size).await,
            "orlen" => Self::generate_orlen_label(config, from, to, size).await,
            _ => Err(ApiError::bad_request("delivery.unsupported_provider")),
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    async fn get_setting(pool: &MySqlPool, key: &str) -> Result<Option<String>, ApiError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT `value` FROM settings WHERE `key` = ? LIMIT 1")
                .bind(key)
                .fetch_optional(pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    fn parse_inpost_response(body: &serde_json::Value) -> Vec<Locker> {
        let items = body["items"]
            .as_array()
            .or_else(|| body["data"].as_array())
            .cloned()
            .unwrap_or_default();

        items
            .iter()
            .filter_map(|item| Self::parse_inpost_point(item))
            .collect()
    }

    fn parse_inpost_point(item: &serde_json::Value) -> Option<Locker> {
        let name = item["name"].as_str()?;
        let address_details = &item["address_details"];
        let location = &item["location"];

        Some(Locker {
            name: name.to_string(),
            address: format!(
                "{} {}",
                address_details["street"].as_str().unwrap_or(""),
                address_details["building_number"].as_str().unwrap_or("")
            )
            .trim()
            .to_string(),
            city: address_details["city"].as_str().map(String::from),
            postal_code: address_details["post_code"].as_str().map(String::from),
            latitude: location["latitude"].as_f64().unwrap_or(0.0),
            longitude: location["longitude"].as_f64().unwrap_or(0.0),
            provider: "inpost".to_string(),
            status: item["status"].as_str().map(String::from),
            opening_hours: item["opening_hours"].as_str().map(String::from),
        })
    }

    async fn generate_inpost_label(
        config: &SharedConfig,
        from: &ShipmentAddress,
        to: &ShipmentAddress,
        size: &str,
    ) -> Result<ShipmentLabel, ApiError> {
        let target_locker = to
            .locker_name
            .as_deref()
            .ok_or_else(|| ApiError::bad_request("delivery.locker_required"))?;

        let payload = serde_json::json!({
            "receiver": {
                "name": to.name,
                "phone": to.phone,
                "email": null,
            },
            "sender": {
                "name": from.name,
                "phone": from.phone,
            },
            "parcels": [{
                "dimensions": {
                    "length": null,
                    "width": null,
                    "height": null,
                    "weight": null,
                },
                "template": size,
            }],
            "service": "inpost_locker_standard",
            "custom_attributes": {
                "target_point": target_locker,
                "sending_method": from.locker_name.as_deref().unwrap_or("dispatch_order"),
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&format!("{}/v1/organizations/shipments", config.inpost_api_base_url))
            .header("Authorization", format!("Bearer {}", config.inpost_api_token))
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("InPost shipment creation failed: {}", body);
            return Err(ApiError::external_service("delivery.shipment_creation_failed"));
        }

        let body: serde_json::Value = resp.json().await?;

        Ok(ShipmentLabel {
            shipment_id: body["id"].as_str().unwrap_or_default().to_string(),
            label_url: body["label_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            tracking_number: body["tracking_number"].as_str().map(String::from),
            provider: "inpost".to_string(),
        })
    }

    async fn generate_orlen_label(
        config: &SharedConfig,
        from: &ShipmentAddress,
        to: &ShipmentAddress,
        size: &str,
    ) -> Result<ShipmentLabel, ApiError> {
        if config.orlen_api_base_url.is_empty() {
            return Err(ApiError::external_service("delivery.orlen_not_configured"));
        }

        let target_locker = to
            .locker_name
            .as_deref()
            .ok_or_else(|| ApiError::bad_request("delivery.locker_required"))?;

        let payload = serde_json::json!({
            "sender": {
                "name": from.name,
                "phone": from.phone,
                "city": from.city,
                "postal_code": from.postal_code,
            },
            "receiver": {
                "name": to.name,
                "phone": to.phone,
                "locker_name": target_locker,
            },
            "parcel_size": size,
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&format!("{}/shipments", config.orlen_api_base_url))
            .header("Authorization", format!("Bearer {}", config.orlen_api_token))
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Orlen shipment creation failed: {}", body);
            return Err(ApiError::external_service("delivery.shipment_creation_failed"));
        }

        let body: serde_json::Value = resp.json().await?;

        Ok(ShipmentLabel {
            shipment_id: body["id"].as_str().unwrap_or_default().to_string(),
            label_url: body["label_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            tracking_number: body["tracking_number"].as_str().map(String::from),
            provider: "orlen".to_string(),
        })
    }
}
