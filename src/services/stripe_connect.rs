use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::config::SharedConfig;
use crate::errors::ApiError;

// ── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateConnectData {
    pub business_type: Option<String>,
    pub country: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectData {
    pub business_type: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub dob_day: Option<i32>,
    pub dob_month: Option<i32>,
    pub dob_year: Option<i32>,
    pub address_line1: Option<String>,
    pub address_city: Option<String>,
    pub address_postal_code: Option<String>,
    pub address_country: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConnectStatus {
    pub account_id: Option<String>,
    pub status: Option<String>,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub onboarded_at: Option<chrono::NaiveDateTime>,
    pub details_submitted: bool,
    pub requirements: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddBankAccountData {
    pub account_holder_name: String,
    pub account_holder_type: Option<String>,
    pub routing_number: Option<String>,
    pub account_number: String,
    pub country: String,
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct BankAccountInfo {
    pub id: String,
    pub bank_name: Option<String>,
    pub last4: String,
    pub country: String,
    pub currency: String,
    pub default_for_currency: bool,
}

// ── Service ─────────────────────────────────────────────────────────────────

pub struct StripeConnectService;

impl StripeConnectService {
    /// Create a Stripe Connect Express account for the user.
    pub async fn create_connect_account(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
        data: CreateConnectData,
    ) -> Result<String, ApiError> {
        // Check if user already has a Connect account
        let existing: (Option<String>,) = sqlx::query_as(
            "SELECT stripe_connect_account_id FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        if let Some(account_id) = existing.0 {
            return Ok(account_id);
        }

        let user: (String, String, String) = sqlx::query_as(
            "SELECT email, first_name, last_name FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        let email = data.email.unwrap_or(user.0);
        let country = data.country.unwrap_or_else(|| "PL".to_string());
        let business_type = data.business_type.unwrap_or_else(|| "individual".to_string());

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.stripe.com/v1/accounts")
            .bearer_auth(&config.stripe_secret_key)
            .form(&[
                ("type", "express"),
                ("country", &country),
                ("email", &email),
                ("business_type", &business_type),
                ("metadata[user_id]", &user_id.to_string()),
                ("capabilities[card_payments][requested]", "true"),
                ("capabilities[transfers][requested]", "true"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Stripe Connect account creation failed: {}", body);
            return Err(ApiError::payment("payment.connect_creation_failed"));
        }

        let body: serde_json::Value = resp.json().await?;
        let account_id = body["id"]
            .as_str()
            .ok_or_else(|| ApiError::payment("payment.stripe_invalid_response"))?
            .to_string();

        sqlx::query(
            r#"
            UPDATE users SET
                stripe_connect_account_id = ?,
                stripe_connect_status = 'pending',
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(&account_id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(account_id)
    }

    /// Update a Stripe Connect account with additional identity / business details.
    pub async fn update_connect_account(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
        data: UpdateConnectData,
    ) -> Result<(), ApiError> {
        let account_id = Self::get_account_id(pool, user_id).await?;

        let mut params: Vec<(String, String)> = Vec::new();

        if let Some(ref email) = data.email {
            params.push(("email".to_string(), email.clone()));
        }
        if let Some(ref first) = data.first_name {
            params.push((
                "individual[first_name]".to_string(),
                first.clone(),
            ));
        }
        if let Some(ref last) = data.last_name {
            params.push((
                "individual[last_name]".to_string(),
                last.clone(),
            ));
        }
        if let Some(ref phone) = data.phone {
            params.push(("individual[phone]".to_string(), phone.clone()));
        }
        if let Some(day) = data.dob_day {
            params.push((
                "individual[dob][day]".to_string(),
                day.to_string(),
            ));
        }
        if let Some(month) = data.dob_month {
            params.push((
                "individual[dob][month]".to_string(),
                month.to_string(),
            ));
        }
        if let Some(year) = data.dob_year {
            params.push((
                "individual[dob][year]".to_string(),
                year.to_string(),
            ));
        }
        if let Some(ref line1) = data.address_line1 {
            params.push((
                "individual[address][line1]".to_string(),
                line1.clone(),
            ));
        }
        if let Some(ref city) = data.address_city {
            params.push((
                "individual[address][city]".to_string(),
                city.clone(),
            ));
        }
        if let Some(ref postal) = data.address_postal_code {
            params.push((
                "individual[address][postal_code]".to_string(),
                postal.clone(),
            ));
        }
        if let Some(ref country) = data.address_country {
            params.push((
                "individual[address][country]".to_string(),
                country.clone(),
            ));
        }

        if params.is_empty() {
            return Ok(());
        }

        let client = reqwest::Client::new();
        let url = format!("https://api.stripe.com/v1/accounts/{}", account_id);
        let resp = client
            .post(&url)
            .bearer_auth(&config.stripe_secret_key)
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Stripe Connect account update failed: {}", body);
            return Err(ApiError::payment("payment.connect_update_failed"));
        }

        Ok(())
    }

    /// Submit identity documents for verification.
    pub async fn submit_documents(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
        document_front_file_id: &str,
        document_back_file_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let account_id = Self::get_account_id(pool, user_id).await?;

        let mut params = vec![(
            "individual[verification][document][front]".to_string(),
            document_front_file_id.to_string(),
        )];

        if let Some(back_id) = document_back_file_id {
            params.push((
                "individual[verification][document][back]".to_string(),
                back_id.to_string(),
            ));
        }

        let client = reqwest::Client::new();
        let url = format!("https://api.stripe.com/v1/accounts/{}", account_id);
        let resp = client
            .post(&url)
            .bearer_auth(&config.stripe_secret_key)
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Stripe document submission failed: {}", body);
            return Err(ApiError::payment("payment.document_submission_failed"));
        }

        Ok(())
    }

    /// Get the current Connect account status, capabilities, and pending requirements.
    pub async fn get_connect_status(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<ConnectStatus, ApiError> {
        let user: (
            Option<String>,
            Option<String>,
            bool,
            bool,
            Option<chrono::NaiveDateTime>,
        ) = sqlx::query_as(
            r#"
            SELECT
                stripe_connect_account_id,
                stripe_connect_status,
                stripe_connect_charges_enabled,
                stripe_connect_payouts_enabled,
                stripe_connect_onboarded_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        let account_id = match user.0 {
            Some(ref id) => id.clone(),
            None => {
                return Ok(ConnectStatus {
                    account_id: None,
                    status: None,
                    charges_enabled: false,
                    payouts_enabled: false,
                    onboarded_at: None,
                    details_submitted: false,
                    requirements: vec![],
                });
            }
        };

        // Fetch live status from Stripe
        let client = reqwest::Client::new();
        let url = format!("https://api.stripe.com/v1/accounts/{}", account_id);
        let resp = client
            .get(&url)
            .bearer_auth(&config.stripe_secret_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            // Fall back to local data
            return Ok(ConnectStatus {
                account_id: Some(account_id),
                status: user.1,
                charges_enabled: user.2,
                payouts_enabled: user.3,
                onboarded_at: user.4,
                details_submitted: false,
                requirements: vec![],
            });
        }

        let body: serde_json::Value = resp.json().await?;
        let charges_enabled = body["charges_enabled"].as_bool().unwrap_or(false);
        let payouts_enabled = body["payouts_enabled"].as_bool().unwrap_or(false);
        let details_submitted = body["details_submitted"].as_bool().unwrap_or(false);

        let requirements: Vec<String> = body["requirements"]["currently_due"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Sync local DB with live Stripe data
        let status_str = if charges_enabled && payouts_enabled {
            "active"
        } else if details_submitted {
            "pending"
        } else {
            "incomplete"
        };

        sqlx::query(
            r#"
            UPDATE users SET
                stripe_connect_status = ?,
                stripe_connect_charges_enabled = ?,
                stripe_connect_payouts_enabled = ?,
                updated_at = NOW()
            WHERE id = ?
            "#,
        )
        .bind(status_str)
        .bind(charges_enabled)
        .bind(payouts_enabled)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(ConnectStatus {
            account_id: Some(account_id),
            status: Some(status_str.to_string()),
            charges_enabled,
            payouts_enabled,
            onboarded_at: user.4,
            details_submitted,
            requirements,
        })
    }

    /// Accept the Stripe Terms of Service.
    pub async fn accept_tos(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<(), ApiError> {
        let account_id = Self::get_account_id(pool, user_id).await?;

        let now_ts = chrono::Utc::now().timestamp().to_string();

        let client = reqwest::Client::new();
        let url = format!("https://api.stripe.com/v1/accounts/{}", account_id);
        let resp = client
            .post(&url)
            .bearer_auth(&config.stripe_secret_key)
            .form(&[
                ("tos_acceptance[date]", now_ts.as_str()),
                ("tos_acceptance[ip]", "0.0.0.0"), // Should be passed from handler
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Stripe TOS acceptance failed: {}", body);
            return Err(ApiError::payment("payment.tos_acceptance_failed"));
        }

        Ok(())
    }

    /// Add an external bank account for payouts.
    pub async fn add_bank_account(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
        data: AddBankAccountData,
    ) -> Result<BankAccountInfo, ApiError> {
        let account_id = Self::get_account_id(pool, user_id).await?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://api.stripe.com/v1/accounts/{}/external_accounts",
            account_id
        );
        let resp = client
            .post(&url)
            .bearer_auth(&config.stripe_secret_key)
            .form(&[
                ("external_account[object]", "bank_account"),
                (
                    "external_account[account_holder_name]",
                    &data.account_holder_name,
                ),
                (
                    "external_account[account_holder_type]",
                    data.account_holder_type.as_deref().unwrap_or("individual"),
                ),
                ("external_account[account_number]", &data.account_number),
                ("external_account[country]", &data.country),
                ("external_account[currency]", &data.currency),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Stripe bank account creation failed: {}", body);
            return Err(ApiError::payment("payment.bank_account_creation_failed"));
        }

        let body: serde_json::Value = resp.json().await?;

        Ok(BankAccountInfo {
            id: body["id"].as_str().unwrap_or_default().to_string(),
            bank_name: body["bank_name"].as_str().map(String::from),
            last4: body["last4"].as_str().unwrap_or("0000").to_string(),
            country: body["country"]
                .as_str()
                .unwrap_or(&data.country)
                .to_string(),
            currency: body["currency"]
                .as_str()
                .unwrap_or(&data.currency)
                .to_string(),
            default_for_currency: body["default_for_currency"]
                .as_bool()
                .unwrap_or(false),
        })
    }

    /// List bank accounts (external accounts) for the user's Connect account.
    pub async fn list_bank_accounts(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
    ) -> Result<Vec<BankAccountInfo>, ApiError> {
        let account_id = Self::get_account_id(pool, user_id).await?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://api.stripe.com/v1/accounts/{}/external_accounts?object=bank_account&limit=10",
            account_id
        );
        let resp = client
            .get(&url)
            .bearer_auth(&config.stripe_secret_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Failed to list Stripe bank accounts: {}", body);
            return Err(ApiError::payment("payment.bank_account_list_failed"));
        }

        let body: serde_json::Value = resp.json().await?;
        let accounts = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|a| BankAccountInfo {
                        id: a["id"].as_str().unwrap_or_default().to_string(),
                        bank_name: a["bank_name"].as_str().map(String::from),
                        last4: a["last4"].as_str().unwrap_or("0000").to_string(),
                        country: a["country"].as_str().unwrap_or("").to_string(),
                        currency: a["currency"].as_str().unwrap_or("").to_string(),
                        default_for_currency: a["default_for_currency"]
                            .as_bool()
                            .unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(accounts)
    }

    /// Remove a bank account from the user's Connect account.
    pub async fn remove_bank_account(
        config: &SharedConfig,
        pool: &MySqlPool,
        user_id: i64,
        bank_account_id: &str,
    ) -> Result<(), ApiError> {
        let account_id = Self::get_account_id(pool, user_id).await?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://api.stripe.com/v1/accounts/{}/external_accounts/{}",
            account_id, bank_account_id
        );
        let resp = client
            .delete(&url)
            .bearer_auth(&config.stripe_secret_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Stripe bank account removal failed: {}", body);
            return Err(ApiError::payment("payment.bank_account_removal_failed"));
        }

        Ok(())
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    async fn get_account_id(pool: &MySqlPool, user_id: i64) -> Result<String, ApiError> {
        let row: (Option<String>,) = sqlx::query_as(
            "SELECT stripe_connect_account_id FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("user.not_found"))?;

        row.0
            .ok_or_else(|| ApiError::bad_request("payment.connect_account_required"))
    }
}
