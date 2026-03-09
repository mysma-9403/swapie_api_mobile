use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::dto::{ApiResponse, PaginatedResponse, PaginationParams};
use crate::errors::ApiError;
use crate::i18n;
use crate::middleware::auth::AuthenticatedUser;
use crate::models::{ActionLog, Module, Permission, Role, Setting, UserSafe};

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

/// Check that the authenticated user has the admin role.
/// Returns `Err(ApiError::forbidden())` if the user is not an admin.
async fn require_admin(pool: &MySqlPool, user_id: i64) -> Result<(), ApiError> {
    let is_admin = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM model_has_roles mhr
         JOIN roles r ON r.id = mhr.role_id
         WHERE mhr.model_id = ? AND mhr.model_type = 'App\\\\Models\\\\User' AND r.name = 'admin'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if is_admin == 0 {
        return Err(ApiError::forbidden());
    }

    Ok(())
}

// ── Request DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BulkDeleteRequest {
    pub ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: Vec<SettingEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SettingEntry {
    pub option_name: String,
    pub option_value: Option<String>,
}

// ── Response DTOs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PermissionGroupResponse {
    pub group: String,
    pub permissions: Vec<Permission>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/users
#[tracing::instrument(skip(pool, auth, query))]
pub async fn list_users(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    require_admin(pool.get_ref(), auth.user_id).await?;

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool.get_ref())
        .await? as u64;

    let users = sqlx::query_as::<_, UserSafe>(
        "SELECT id, email, username, first_name, last_name, description, phone_number,
         avatar_id, language, locker_id, stripe_customer_id, stripe_connect_account_id,
         stripe_connect_status, stripe_connect_charges_enabled, stripe_connect_payouts_enabled,
         stripe_connect_onboarded_at, privacy_policy_accepted, terms_of_service_accepted,
         marketing_emails_accepted, consents_accepted_at, google_id, facebook_id,
         social_provider, social_provider_id, average_rating, review_count,
         activation_code_expires_at, last_unread_notification_at, preferred_item_types,
         default_inpost_locker, email_verified_at, created_at, updated_at
         FROM users ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(query.per_page())
    .bind(query.offset())
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(users, query.page(), query.per_page(), total)))
}

/// POST /api/v1/users/bulk-delete
#[tracing::instrument(skip(pool, auth, body))]
pub async fn bulk_delete_users(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<BulkDeleteRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    require_admin(pool.get_ref(), auth.user_id).await?;

    if body.ids.is_empty() {
        return Err(ApiError::bad_request("general.no_ids_provided"));
    }

    // Don't allow deleting yourself
    if body.ids.contains(&auth.user_id) {
        return Err(ApiError::bad_request("users.cannot_delete_self"));
    }

    let placeholders = body.ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("DELETE FROM users WHERE id IN ({})", placeholders);
    let mut query = sqlx::query(&sql);
    for id in &body.ids {
        query = query.bind(id);
    }
    let result = query.execute(pool.get_ref()).await?;

    let deleted = result.rows_affected();
    Ok(HttpResponse::Ok().json(ApiResponse::success(
        serde_json::json!({"deleted": deleted}),
        i18n::t(&lang, "users.bulk_deleted"),
    )))
}

/// GET /api/v1/roles
#[tracing::instrument(skip(pool, auth))]
pub async fn list_roles(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let roles = sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name ASC")
        .fetch_all(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(roles, i18n::t(&lang, "general.success"))))
}

/// POST /api/v1/roles/delete/bulk-delete
#[tracing::instrument(skip(pool, auth, body))]
pub async fn bulk_delete_roles(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<BulkDeleteRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    if body.ids.is_empty() {
        return Err(ApiError::bad_request("general.no_ids_provided"));
    }

    let placeholders = body.ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("DELETE FROM roles WHERE id IN ({})", placeholders);
    let mut query = sqlx::query(&sql);
    for id in &body.ids {
        query = query.bind(id);
    }
    let result = query.execute(pool.get_ref()).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(
        serde_json::json!({"deleted": result.rows_affected()}),
        i18n::t(&lang, "admin.roles_deleted"),
    )))
}

/// GET /api/v1/permissions
#[tracing::instrument(skip(pool, auth))]
pub async fn list_permissions(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let permissions = sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY name ASC",
    )
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(permissions, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/permissions/groups
#[tracing::instrument(skip(pool, auth))]
pub async fn get_permission_groups(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let permissions = sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY name ASC",
    )
    .fetch_all(pool.get_ref())
    .await?;

    // Group permissions by prefix (e.g. "users.create" -> "users" group)
    let mut groups: std::collections::HashMap<String, Vec<Permission>> =
        std::collections::HashMap::new();

    for perm in permissions {
        let group = perm
            .name
            .split('.')
            .next()
            .unwrap_or("general")
            .to_string();
        groups.entry(group).or_default().push(perm);
    }

    let data: Vec<PermissionGroupResponse> = groups
        .into_iter()
        .map(|(group, permissions)| PermissionGroupResponse { group, permissions })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(data, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/permissions/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_permission(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let permission_id = path.into_inner();

    let permission = sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions WHERE id = ?",
    )
    .bind(permission_id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("admin.permission_not_found"))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(permission, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/action-logs
#[tracing::instrument(skip(pool, auth, query))]
pub async fn list_action_logs(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM action_logs")
        .fetch_one(pool.get_ref())
        .await? as u64;

    let logs = sqlx::query_as::<_, ActionLog>(
        "SELECT * FROM action_logs ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(query.per_page())
    .bind(query.offset())
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(PaginatedResponse::new(logs, query.page(), query.per_page(), total)))
}

/// GET /api/v1/action-logs/{id}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_action_log(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let log_id = path.into_inner();

    let log = sqlx::query_as::<_, ActionLog>(
        "SELECT * FROM action_logs WHERE id = ?",
    )
    .bind(log_id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("admin.action_log_not_found"))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(log, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/settings
#[tracing::instrument(skip(pool, auth))]
pub async fn get_settings(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let settings = sqlx::query_as::<_, Setting>(
        "SELECT * FROM settings ORDER BY option_name ASC",
    )
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(settings, i18n::t(&lang, "general.success"))))
}

/// PUT /api/v1/settings
#[tracing::instrument(skip(pool, auth, body))]
pub async fn update_settings(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    body: web::Json<UpdateSettingsRequest>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let now = chrono::Utc::now().naive_utc();

    for entry in &body.settings {
        sqlx::query(
            "INSERT INTO settings (option_name, option_value, autoload, created_at, updated_at)
             VALUES (?, ?, true, ?, ?)
             ON DUPLICATE KEY UPDATE option_value = VALUES(option_value), updated_at = VALUES(updated_at)",
        )
        .bind(&entry.option_name)
        .bind(&entry.option_value)
        .bind(now)
        .bind(now)
        .execute(pool.get_ref())
        .await?;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "admin.settings_updated"))))
}

/// GET /api/v1/settings/{key}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_setting(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let key = path.into_inner();

    let setting = sqlx::query_as::<_, Setting>(
        "SELECT * FROM settings WHERE option_name = ?",
    )
    .bind(&key)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("admin.setting_not_found"))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(setting, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/modules
#[tracing::instrument(skip(pool, auth))]
pub async fn list_modules(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);

    let modules = sqlx::query_as::<_, Module>(
        "SELECT * FROM modules ORDER BY name ASC",
    )
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(modules, i18n::t(&lang, "general.success"))))
}

/// GET /api/v1/modules/{name}
#[tracing::instrument(skip(pool, auth))]
pub async fn get_module(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let name = path.into_inner();

    let module = sqlx::query_as::<_, Module>(
        "SELECT * FROM modules WHERE name = ?",
    )
    .bind(&name)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("admin.module_not_found"))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(module, i18n::t(&lang, "general.success"))))
}

/// PATCH /api/v1/modules/{name}/toggle-status
#[tracing::instrument(skip(pool, auth))]
pub async fn toggle_module(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let name = path.into_inner();

    let module = sqlx::query_as::<_, Module>(
        "SELECT * FROM modules WHERE name = ?",
    )
    .bind(&name)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ApiError::not_found("admin.module_not_found"))?;

    let new_status = !module.is_active;

    sqlx::query("UPDATE modules SET is_active = ?, updated_at = NOW() WHERE name = ?")
        .bind(new_status)
        .bind(&name)
        .execute(pool.get_ref())
        .await?;

    let updated_module = sqlx::query_as::<_, Module>(
        "SELECT * FROM modules WHERE name = ?",
    )
    .bind(&name)
    .fetch_one(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(updated_module, i18n::t(&lang, "admin.module_toggled"))))
}

/// DELETE /api/v1/modules/{name}
#[tracing::instrument(skip(pool, auth))]
pub async fn delete_module(
    req: HttpRequest,
    pool: web::Data<MySqlPool>,
    auth: AuthenticatedUser,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let lang = lang_from_req(&req);
    let name = path.into_inner();

    let result = sqlx::query("DELETE FROM modules WHERE name = ?")
        .bind(&name)
        .execute(pool.get_ref())
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("admin.module_not_found"));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::message(i18n::t(&lang, "admin.module_deleted"))))
}
