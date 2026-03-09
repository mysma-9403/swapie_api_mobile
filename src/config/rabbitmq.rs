use super::env_or;

#[derive(Debug, Clone)]
pub struct RabbitMQConfig {
    pub url: String,
    pub default_queue: String,
    pub emails_queue: String,
    pub sms_queue: String,
    pub shipments_queue: String,
    pub notifications_queue: String,
}

impl RabbitMQConfig {
    pub fn from_env() -> Self {
        let host = env_or("RABBITMQ_HOST", "127.0.0.1");
        let port = env_or("RABBITMQ_PORT", "5672");
        let user = env_or("RABBITMQ_USER", "guest");
        let password = env_or("RABBITMQ_PASSWORD", "guest");
        let vhost = env_or("RABBITMQ_VHOST", "/");

        Self {
            url: format!("amqp://{}:{}@{}:{}/{}", user, password, host, port, vhost),
            default_queue: "default".to_string(),
            emails_queue: "emails".to_string(),
            sms_queue: "sms".to_string(),
            shipments_queue: "shipments".to_string(),
            notifications_queue: "notifications".to_string(),
        }
    }
}
