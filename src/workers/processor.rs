use futures::StreamExt;
use lapin::{options::*, types::FieldTable};
use sqlx::MySqlPool;
use std::sync::Arc;

use crate::config::Config;
use crate::services::queue::QueueJob;

pub struct JobProcessor {
    config: Arc<Config>,
    pool: MySqlPool,
}

impl JobProcessor {
    pub fn new(config: Arc<Config>, pool: MySqlPool) -> Self {
        Self { config, pool }
    }

    /// Start consuming from a specific queue. Runs forever.
    pub async fn run(&self, queue_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = lapin::Connection::connect(
            &self.config.rabbitmq.url,
            lapin::ConnectionProperties::default(),
        )
        .await?;

        let channel = conn.create_channel().await?;

        // Declare queue
        channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        // Set prefetch to 1 (process one job at a time)
        channel
            .basic_qos(1, BasicQosOptions::default())
            .await?;

        let mut consumer = channel
            .basic_consume(
                queue_name,
                &format!("swapie-worker-{}", queue_name),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        tracing::info!("Worker started consuming queue: {}", queue_name);

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let job: Result<QueueJob, _> = serde_json::from_slice(&delivery.data);
                    match job {
                        Ok(job) => {
                            tracing::info!(
                                "Processing job: {} (attempt {}/{})",
                                job.job_type,
                                job.attempts + 1,
                                job.max_attempts
                            );

                            match self.process_job(&job).await {
                                Ok(()) => {
                                    delivery.ack(BasicAckOptions::default()).await?;
                                    tracing::info!(
                                        "Job '{}' completed successfully",
                                        job.job_type
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("Job '{}' failed: {}", job.job_type, e);
                                    if job.attempts + 1 < job.max_attempts {
                                        // Nack and requeue for retry
                                        delivery
                                            .nack(BasicNackOptions {
                                                requeue: true,
                                                ..Default::default()
                                            })
                                            .await?;
                                    } else {
                                        // Max attempts reached, move to failed_jobs table
                                        tracing::error!(
                                            "Job '{}' exceeded max attempts, moving to failed_jobs",
                                            job.job_type
                                        );
                                        self.store_failed_job(&job, &e.to_string()).await;
                                        delivery.ack(BasicAckOptions::default()).await?;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to deserialize job: {}", e);
                            delivery.ack(BasicAckOptions::default()).await?;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Consumer error: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn process_job(
        &self,
        job: &QueueJob,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match job.job_type.as_str() {
            "ProcessTradeShipment" => self.process_trade_shipment(&job.payload).await,
            "SendShipmentLabelEmail" => self.send_shipment_label_email(&job.payload).await,
            "SendSms" => self.send_sms(&job.payload).await,
            "SendTemplateEmail" => self.send_template_email(&job.payload).await,
            "SendUnreadNotifications" => self.send_unread_notifications(&job.payload).await,
            "ProcessLockerBatch" => self.process_locker_batch(&job.payload).await,
            other => {
                tracing::warn!("Unknown job type: {}", other);
                Ok(())
            }
        }
    }

    /// Process trade shipment - creates shipment via InPost/Orlen API
    async fn process_trade_shipment(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let trade_id = payload["trade_id"].as_i64().ok_or("missing trade_id")?;
        let direction = payload["direction"].as_str().ok_or("missing direction")?;
        let sender_id = payload["sender_id"].as_i64().ok_or("missing sender_id")?;
        let _receiver_id = payload["receiver_id"].as_i64().ok_or("missing receiver_id")?;
        let provider = payload["provider"].as_str().ok_or("missing provider")?;
        let sender_locker = payload["sender_locker"]
            .as_str()
            .ok_or("missing sender_locker")?;
        let receiver_locker = payload["receiver_locker"]
            .as_str()
            .ok_or("missing receiver_locker")?;

        tracing::info!(
            "Creating shipment for trade {} direction {}",
            trade_id,
            direction
        );

        // Call InPost/Orlen API to create shipment
        let client = reqwest::Client::new();
        let (api_url, api_token) = match provider {
            "inpost" => (
                &self.config.inpost_api_base_url,
                &self.config.inpost_api_token,
            ),
            "orlen" => (
                &self.config.orlen_api_base_url,
                &self.config.orlen_api_token,
            ),
            _ => return Err("Unknown provider".into()),
        };

        let response = client
            .post(format!(
                "{}/v1/organizations/default/shipments",
                api_url
            ))
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&serde_json::json!({
                "receiver": { "locker_name": receiver_locker },
                "sender": { "locker_name": sender_locker },
                "parcels": [{"dimensions": {"length": 30, "width": 20, "height": 10}, "weight": {"amount": 1}}]
            }))
            .send()
            .await?;

        let shipment: serde_json::Value = response.json().await?;
        let shipment_id = shipment["id"].as_str().unwrap_or("").to_string();

        // Save to trade
        let shipment_col = match direction {
            "initiator_to_recipient" => "initiator_to_recipient_shipment_id",
            "recipient_to_initiator" => "recipient_to_initiator_shipment_id",
            _ => return Err("Invalid direction".into()),
        };

        let query = format!(
            "UPDATE trades SET {} = ?, updated_at = NOW() WHERE id = ?",
            shipment_col
        );
        sqlx::query(&query)
            .bind(&shipment_id)
            .bind(trade_id)
            .execute(&self.pool)
            .await?;

        // Get sender email for label email
        let sender_email =
            sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = ?")
                .bind(sender_id)
                .fetch_one(&self.pool)
                .await?;

        // Dispatch label email job
        let queue_service = crate::services::queue::QueueService::new(self.config.clone())
            .await
            .map_err(|e| format!("Queue error: {:?}", e))?;
        queue_service
            .dispatch_send_label_email(trade_id, direction, &shipment_id, sender_id, &sender_email)
            .await
            .map_err(|e| format!("Dispatch error: {:?}", e))?;

        tracing::info!(
            "Shipment {} created for trade {}, label email dispatched",
            shipment_id,
            trade_id
        );
        Ok(())
    }

    /// Generate label PDF and send via email
    async fn send_shipment_label_email(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let trade_id = payload["trade_id"].as_i64().ok_or("missing trade_id")?;
        let direction = payload["direction"].as_str().ok_or("missing direction")?;
        let shipment_id = payload["shipment_id"]
            .as_str()
            .ok_or("missing shipment_id")?;
        let _user_id = payload["user_id"].as_i64().ok_or("missing user_id")?;
        let email = payload["email"].as_str().ok_or("missing email")?;

        tracing::info!(
            "Generating label for shipment {} (trade {})",
            shipment_id,
            trade_id
        );

        // Generate label PDF via InPost API
        let client = reqwest::Client::new();
        let label_response = client
            .get(format!(
                "{}/v1/organizations/default/shipments/{}/label",
                self.config.inpost_api_base_url, shipment_id
            ))
            .header(
                "Authorization",
                format!("Bearer {}", self.config.inpost_api_token),
            )
            .header("Accept", "application/pdf")
            .send()
            .await?;

        let label_bytes = label_response.bytes().await?;

        // Upload to S3/Spaces
        let key = format!("labels/trade_{}/{}.pdf", trade_id, shipment_id);
        let upload_url = format!(
            "{}/{}/{}",
            self.config.s3_endpoint, self.config.s3_bucket, key
        );

        client
            .put(&upload_url)
            .header("Content-Type", "application/pdf")
            .header("x-amz-acl", "public-read")
            .body(label_bytes)
            .send()
            .await?;

        let label_url = format!("{}/{}", self.config.s3_url, key);

        // Update trade with label URL
        let label_col = match direction {
            "initiator_to_recipient" => "initiator_to_recipient_label_url",
            "recipient_to_initiator" => "recipient_to_initiator_label_url",
            _ => return Err("Invalid direction".into()),
        };
        let query = format!(
            "UPDATE trades SET {} = ?, updated_at = NOW() WHERE id = ?",
            label_col
        );
        sqlx::query(&query)
            .bind(&label_url)
            .bind(trade_id)
            .execute(&self.pool)
            .await?;

        // Send email with label link
        tracing::info!("Sending label email to {} for trade {}", email, trade_id);
        let email_client = reqwest::Client::new();
        let _ = email_client
            .post(format!("https://{}/api/send", self.config.smtp_host))
            .json(&serde_json::json!({
                "from": self.config.smtp_from_email,
                "to": email,
                "subject": "Twoja etykieta nadawcza - Swapie",
                "html": format!(
                    "<p>Twoja etykieta nadawcza jest gotowa.</p><p><a href='{}'>Pobierz etykietę PDF</a></p>",
                    label_url
                )
            }))
            .send()
            .await;

        Ok(())
    }

    /// Send SMS via external provider
    async fn send_sms(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let phone_number = payload["phone_number"]
            .as_str()
            .ok_or("missing phone_number")?;
        let message = payload["message"].as_str().ok_or("missing message")?;

        tracing::info!(
            "Sending SMS to {}...",
            &phone_number[..6.min(phone_number.len())]
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&self.config.sms_api_url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.sms_api_key),
            )
            .json(&serde_json::json!({
                "phone_number": phone_number,
                "message": message,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("SMS API returned {}: {}", status, body).into());
        }

        Ok(())
    }

    /// Send template email
    async fn send_template_email(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let to_email = payload["to_email"].as_str().ok_or("missing to_email")?;
        let template_key = payload["template_key"]
            .as_str()
            .ok_or("missing template_key")?;
        let variables = &payload["variables"];

        // Load template from DB
        let template = sqlx::query_as::<_, (String, String, String)>(
            "SELECT subject, html_body, text_body FROM email_templates WHERE `key` = ? AND is_active = true",
        )
        .bind(template_key)
        .fetch_optional(&self.pool)
        .await?;

        let (subject, html_body, _text_body) = match template {
            Some(t) => t,
            None => {
                tracing::warn!("Email template '{}' not found, skipping", template_key);
                return Ok(());
            }
        };

        // Simple variable replacement in template
        let mut rendered = html_body;
        if let Some(vars) = variables.as_object() {
            for (key, value) in vars {
                let placeholder = format!("{{{{{}}}}}", key);
                let replacement = value.as_str().unwrap_or("");
                rendered = rendered.replace(&placeholder, replacement);
            }
        }

        tracing::info!(
            "Sending template email '{}' to {}",
            template_key,
            to_email
        );

        let client = reqwest::Client::new();
        let _ = client
            .post(format!("https://{}/api/send", self.config.smtp_host))
            .json(&serde_json::json!({
                "from": self.config.smtp_from_email,
                "to": to_email,
                "subject": subject,
                "html": rendered,
            }))
            .send()
            .await;

        Ok(())
    }

    /// Send unread notifications batch (push notifications)
    async fn send_unread_notifications(
        &self,
        _payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Processing unread notifications batch");

        // Find users with unread messages that haven't been push-notified
        let users_with_unread: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT m.sender_id, COUNT(*) as unread_count
             FROM messages m
             JOIN trades t ON t.id = m.trade_id
             WHERE m.is_read = false AND m.push_sent = false AND m.is_system_message = false
             GROUP BY m.sender_id",
        )
        .fetch_all(&self.pool)
        .await?;

        for (user_id, unread_count) in users_with_unread {
            // Get device tokens
            let tokens: Vec<String> = sqlx::query_scalar(
                "SELECT fcm_token FROM device_tokens WHERE user_id = ? AND is_active = true",
            )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

            if tokens.is_empty() {
                continue;
            }

            // Send FCM push to each device
            let client = reqwest::Client::new();
            for token in &tokens {
                let _ = client
                    .post(format!(
                        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
                        self.config.fcm_project_id
                    ))
                    .header(
                        "Authorization",
                        format!("Bearer {}", self.config.fcm_server_key),
                    )
                    .json(&serde_json::json!({
                        "message": {
                            "token": token,
                            "notification": {
                                "title": "Nieprzeczytane wiadomości",
                                "body": format!("Masz {} nieprzeczytanych wiadomości", unread_count),
                            },
                            "data": {
                                "type": "unread_messages",
                                "count": unread_count.to_string(),
                            }
                        }
                    }))
                    .send()
                    .await;
            }

            // Mark messages as push_sent
            sqlx::query(
                "UPDATE messages SET push_sent = true, push_sent_at = NOW()
                 WHERE sender_id = ? AND is_read = false AND push_sent = false",
            )
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Process locker batch (sync from InPost/Orlen API)
    async fn process_locker_batch(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = payload["provider"].as_str().ok_or("missing provider")?;
        let data = payload["data"].as_array().ok_or("missing data array")?;

        tracing::info!(
            "Processing {} locker batch: {} entries",
            provider,
            data.len()
        );

        for locker_data in data {
            let name = locker_data["name"].as_str().unwrap_or("");
            let address = locker_data["address"]
                .as_str()
                .or_else(|| locker_data["address_details"].as_str())
                .unwrap_or("");
            let city = locker_data["city"].as_str().unwrap_or("");
            let zip = locker_data["zip_code"]
                .as_str()
                .or_else(|| locker_data["post_code"].as_str())
                .unwrap_or("");
            let lat = locker_data["latitude"].as_f64().unwrap_or(0.0);
            let lng = locker_data["longitude"].as_f64().unwrap_or(0.0);
            let description = locker_data["description"]
                .as_str()
                .or_else(|| locker_data["location_description"].as_str());

            if name.is_empty() {
                continue;
            }

            sqlx::query(
                "INSERT INTO lockers (name, provider, address, city, zip_code, latitude, longitude, description, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, true, NOW(), NOW())
                 ON DUPLICATE KEY UPDATE address = VALUES(address), city = VALUES(city), zip_code = VALUES(zip_code),
                 latitude = VALUES(latitude), longitude = VALUES(longitude), description = VALUES(description),
                 is_active = true, updated_at = NOW()",
            )
            .bind(name)
            .bind(provider)
            .bind(address)
            .bind(city)
            .bind(zip)
            .bind(lat)
            .bind(lng)
            .bind(description)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Store a failed job in the database for later inspection
    async fn store_failed_job(&self, job: &QueueJob, error: &str) {
        let _ = sqlx::query(
            "INSERT INTO failed_jobs (queue, payload, exception, failed_at) VALUES (?, ?, ?, NOW())",
        )
        .bind(&job.queue)
        .bind(serde_json::to_string(job).unwrap_or_default())
        .bind(error)
        .execute(&self.pool)
        .await;
    }
}
