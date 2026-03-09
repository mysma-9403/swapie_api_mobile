use actix_web::web::Data;
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest};
use futures::future::{ok, Ready};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::config::SharedConfig;

// ── JWT Claims ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// User ID (stored as string in JWT `sub` field).
    pub sub: String,
    pub email: String,
    pub username: String,
    pub exp: usize,
    pub iat: usize,
}

// ── Authenticated user extractor ────────────────────────────────────────
//
// Used as a handler parameter to enforce authentication. If the JWT is
// valid the handler receives an `AuthenticatedUser`; otherwise a 401 is
// returned automatically.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub email: String,
    pub username: String,
}

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        // First check if an upstream middleware already inserted the user.
        if let Some(user) = req.extensions().get::<AuthenticatedUser>() {
            return ok(user.clone());
        }

        // Otherwise, try to validate the JWT from the Authorization header.
        let config = req
            .app_data::<Data<SharedConfig>>()
            .map(|c| c.get_ref().clone());

        let config = match config {
            Some(c) => c,
            None => {
                tracing::error!("SharedConfig not found in app data");
                return futures::future::err(actix_web::error::ErrorInternalServerError(
                    serde_json::json!({
                        "success": false,
                        "message": "Internal server error"
                    })
                    .to_string(),
                ));
            }
        };

        let auth_header = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|v| v.to_string());

        let token = match auth_header {
            Some(t) => t,
            None => {
                return futures::future::err(actix_web::error::ErrorUnauthorized(
                    serde_json::json!({
                        "success": false,
                        "message": "Missing or invalid Authorization header"
                    })
                    .to_string(),
                ));
            }
        };

        match verify_jwt(&config.jwt_secret, &token) {
            Ok(claims) => {
                let user_id: i64 = match claims.sub.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        return futures::future::err(actix_web::error::ErrorUnauthorized(
                            serde_json::json!({
                                "success": false,
                                "message": "Invalid token claims"
                            })
                            .to_string(),
                        ));
                    }
                };

                let user = AuthenticatedUser {
                    user_id,
                    email: claims.email,
                    username: claims.username,
                };

                // Store in extensions so subsequent extractors can find it.
                req.extensions_mut().insert(user.clone());
                ok(user)
            }
            Err(err) => {
                tracing::warn!("JWT validation failed: {:?}", err);
                futures::future::err(actix_web::error::ErrorUnauthorized(
                    serde_json::json!({
                        "success": false,
                        "message": "Invalid or expired token"
                    })
                    .to_string(),
                ))
            }
        }
    }
}

// ── Optional user extractor (for endpoints that work with or without auth) ──

#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<AuthenticatedUser>);

impl FromRequest for OptionalUser {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        // Try the same JWT validation but return None on failure instead of an error.
        if let Some(user) = req.extensions().get::<AuthenticatedUser>() {
            return ok(OptionalUser(Some(user.clone())));
        }

        let config = req
            .app_data::<Data<SharedConfig>>()
            .map(|c| c.get_ref().clone());

        let config = match config {
            Some(c) => c,
            None => return ok(OptionalUser(None)),
        };

        let token = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|v| v.to_string());

        let token = match token {
            Some(t) => t,
            None => return ok(OptionalUser(None)),
        };

        match verify_jwt(&config.jwt_secret, &token) {
            Ok(claims) => {
                if let Ok(user_id) = claims.sub.parse::<i64>() {
                    let user = AuthenticatedUser {
                        user_id,
                        email: claims.email,
                        username: claims.username,
                    };
                    req.extensions_mut().insert(user.clone());
                    ok(OptionalUser(Some(user)))
                } else {
                    ok(OptionalUser(None))
                }
            }
            Err(_) => ok(OptionalUser(None)),
        }
    }
}

// ── JWT helper functions ────────────────────────────────────────────────

pub fn generate_jwt(secret: &str, expiration_days: i64, user_id: i64, email: &str, username: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now();
    let exp = (now + chrono::Duration::days(expiration_days)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        username: username.to_string(),
        exp,
        iat,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn verify_jwt(secret: &str, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}
