use actix_web::{HttpMessage, dev::ServiceRequest, http::Error};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user id
    pub iat: usize,
    pub exp: usize,
    pub iss: String,
}

fn secret() -> Result<String, String> {
    env::var("JWT_SECRET").map_err(|_| "JWT_SECRET not set".to_string())
}

fn issuer() -> String {
    env::var("JWT_ISSUER").unwrap_or_else(|_| "worddistancegame".to_string())
}

fn expiry_duration() -> Duration {
    let mins = env::var("JWT_EXP_MINUTES")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(60);
    Duration::minutes(mins)
}

pub fn generate_token(user_id: &uuid::Uuid) -> Result<String, String> {
    let now = Utc::now();
    let exp = now + expiry_duration();

    let claims = Claims {
        sub: user_id.to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
        iss: issuer(),
    };

    let key = EncodingKey::from_secret(secret()?.as_bytes());
    encode(&Header::new(Algorithm::HS256), &claims, &key).map_err(|e| e.to_string())
}

pub fn decode_token(token: &str) -> Result<Claims, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[issuer()]);
    let key = DecodingKey::from_secret(secret()?.as_bytes());
    decode::<Claims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| e.to_string())
}

pub async fn validator(
    mut req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (actix_web::error::Error, ServiceRequest)> {
    let token = credentials.token();
    match decode_token(token) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            Ok(req)
        }
        Err(_) => {
            let err = actix_web::error::ErrorUnauthorized("invalid or expired token");
            Err((err, req))
        }
    }
}
