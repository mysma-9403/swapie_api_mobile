use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::errors::ApiError;

/// Represents a job that can be queued for async processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueJob {
    pub job_type: String,
    pub payload: serde_json::Value,
    pub queue: String,
    pub attempts: u32,
    pub max_attempts: u32,
}

/// Queue service for publishing and consuming RabbitMQ messages.
#[derive(Clone)]
pub struct QueueService {
    connection: Arc<RwLock<Option<Connection>>>,
    config: Arc<Config>,
}

impl QueueService {
    pub async fn new(config: Arc<Config>) -> Result<Self, ApiError> {
        let service = Self {
            connection: Arc::new(RwLock::new(None)),
            config,
        };
        service.connect().await?;
        Ok(service)
    }

    async fn connect(&self) -> Result<(), ApiError> {
        let conn = Connection::connect(
            &self.config.rabbitmq.url,
            ConnectionProperties::default(),
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to RabbitMQ: {}", e);
            ApiError::internal()
        })?;

        let mut lock = self.connection.write().await;
        *lock = Some(conn);
        tracing::info!("Connected to RabbitMQ");
        Ok(())
    }

    async fn get_channel(&self) -> Result<Channel, ApiError> {
        let lock = self.connection.read().await;
        let conn = lock.as_ref().ok_or_else(|| ApiError::internal())?;
        conn.create_channel().await.map_err(|e| {
            tracing::error!("Failed to create RabbitMQ channel: {}", e);
            ApiError::internal()
        })
    }

    /// Declare all required queues on startup.
    pub async fn declare_queues(&self) -> Result<(), ApiError> {
        let channel = self.get_channel().await?;
        let queues = [
            &self.config.rabbitmq.default_queue,
            &self.config.rabbitmq.emails_queue,
            &self.config.rabbitmq.sms_queue,
            &self.config.rabbitmq.shipments_queue,
            &self.config.rabbitmq.notifications_queue,
        ];
        for queue_name in queues {
            channel
                .queue_declare(
                    queue_name,
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .map_err(|e| {
                    tracing::error!("Failed to declare queue '{}': {}", queue_name, e);
                    ApiError::internal()
                })?;
            tracing::info!("Queue '{}' declared", queue_name);
        }
        Ok(())
    }

    /// Publish a job to a specific queue.
    pub async fn dispatch(&self, job: QueueJob) -> Result<(), ApiError> {
        let channel = self.get_channel().await?;
        let payload =
            serde_json::to_vec(&job).map_err(|_| ApiError::internal())?;

        channel
            .basic_publish(
                "",
                &job.queue,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_delivery_mode(2) // persistent
                    .with_content_type("application/json".into()),
            )
            .await
            .map_err(|e| {
                tracing::error!("Failed to publish to queue '{}': {}", job.queue, e);
                ApiError::internal()
            })?
            .await
            .map_err(|e| {
                tracing::error!("Publisher confirm failed: {}", e);
                ApiError::internal()
            })?;

        tracing::debug!("Job '{}' dispatched to queue '{}'", job.job_type, job.queue);
        Ok(())
    }

    /// Helper: dispatch a shipment processing job
    pub async fn dispatch_process_shipment(
        &self,
        trade_id: i64,
        direction: &str,
        sender_id: i64,
        receiver_id: i64,
        provider: &str,
        sender_locker: &str,
        receiver_locker: &str,
    ) -> Result<(), ApiError> {
        self.dispatch(QueueJob {
            job_type: "ProcessTradeShipment".to_string(),
            payload: serde_json::json!({
                "trade_id": trade_id,
                "direction": direction,
                "sender_id": sender_id,
                "receiver_id": receiver_id,
                "provider": provider,
                "sender_locker": sender_locker,
                "receiver_locker": receiver_locker,
            }),
            queue: self.config.rabbitmq.shipments_queue.clone(),
            attempts: 0,
            max_attempts: 3,
        })
        .await
    }

    /// Helper: dispatch a shipment label email job
    pub async fn dispatch_send_label_email(
        &self,
        trade_id: i64,
        direction: &str,
        shipment_id: &str,
        user_id: i64,
        email: &str,
    ) -> Result<(), ApiError> {
        self.dispatch(QueueJob {
            job_type: "SendShipmentLabelEmail".to_string(),
            payload: serde_json::json!({
                "trade_id": trade_id,
                "direction": direction,
                "shipment_id": shipment_id,
                "user_id": user_id,
                "email": email,
            }),
            queue: self.config.rabbitmq.emails_queue.clone(),
            attempts: 0,
            max_attempts: 3,
        })
        .await
    }

    /// Helper: dispatch an SMS job
    pub async fn dispatch_send_sms(
        &self,
        phone_number: &str,
        message: &str,
    ) -> Result<(), ApiError> {
        self.dispatch(QueueJob {
            job_type: "SendSms".to_string(),
            payload: serde_json::json!({
                "phone_number": phone_number,
                "message": message,
            }),
            queue: self.config.rabbitmq.sms_queue.clone(),
            attempts: 0,
            max_attempts: 3,
        })
        .await
    }

    /// Helper: dispatch a template email job
    pub async fn dispatch_send_email(
        &self,
        to_email: &str,
        template_key: &str,
        variables: serde_json::Value,
    ) -> Result<(), ApiError> {
        self.dispatch(QueueJob {
            job_type: "SendTemplateEmail".to_string(),
            payload: serde_json::json!({
                "to_email": to_email,
                "template_key": template_key,
                "variables": variables,
            }),
            queue: self.config.rabbitmq.emails_queue.clone(),
            attempts: 0,
            max_attempts: 3,
        })
        .await
    }

    /// Helper: dispatch unread notifications batch job
    pub async fn dispatch_send_unread_notifications(&self) -> Result<(), ApiError> {
        self.dispatch(QueueJob {
            job_type: "SendUnreadNotifications".to_string(),
            payload: serde_json::json!({}),
            queue: self.config.rabbitmq.notifications_queue.clone(),
            attempts: 0,
            max_attempts: 3,
        })
        .await
    }

    /// Helper: dispatch locker sync batch job
    pub async fn dispatch_sync_lockers(
        &self,
        provider: &str,
        batch_data: serde_json::Value,
    ) -> Result<(), ApiError> {
        self.dispatch(QueueJob {
            job_type: "ProcessLockerBatch".to_string(),
            payload: serde_json::json!({
                "provider": provider,
                "data": batch_data,
            }),
            queue: self.config.rabbitmq.default_queue.clone(),
            attempts: 0,
            max_attempts: 1,
        })
        .await
    }
}
