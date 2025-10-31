use actix_web::{HttpResponse, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        jwt::generate_token,
        ldap::{employee_ldap_login, stdent_ldap_login},
    },
    db::{get_connection, types::DbError},
    models::user::{NewUser, PublicUser, User},
};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: PublicUser,
    pub created: bool,
    pub token: String,
}

#[post("/login/student")]
pub async fn login_student(body: web::Json<LoginRequest>) -> actix_web::Result<HttpResponse> {
    login_common(stdent_ldap_login, body.0).await
}

#[post("/login/employee")]
pub async fn login_employee(body: web::Json<LoginRequest>) -> actix_web::Result<HttpResponse> {
    login_common(employee_ldap_login, body.0).await
}

async fn login_common<Fut>(
    ldap_fn: fn(String, String) -> Fut,
    body: LoginRequest,
) -> actix_web::Result<HttpResponse>
where
    Fut: std::future::Future<Output = ldap3::result::Result<Option<String>>>,
{
    // authenticate via LDAP
    let maybe_dn = match ldap_fn(body.username.clone(), body.password).await {
        Ok(dn) => dn,
        Err(e) => {
            return Ok(HttpResponse::BadGateway().json(serde_json::json!({
                "error": format!("ldap connection failed: {e}")
            })));
        }
    };

    let dn = match maybe_dn {
        Some(dn) => dn,
        None => {
            return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "invalid credentials"
            })));
        }
    };

    // get DB connection
    let conn = match get_connection() {
        Ok(c) => c,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("database connection error: {e}")
            })));
        }
    };

    // find or create user
    let mut created = false;
    let user = match User::get_by_ldap(&conn, &dn) {
        Ok(u) => u,
        Err(DbError::Sql(rusqlite::Error::QueryReturnedNoRows)) => {
            created = true;
            match User::insert(
                &conn,
                NewUser {
                    ldap_id: dn.clone(),
                    name: body.username,
                },
            ) {
                Ok(u) => u,
                Err(e) => {
                    return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("failed to insert user: {e}")
                    })));
                }
            }
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("database query failed: {e}")
            })));
        }
    };

    // issue JWT
    let token = match generate_token(&user.id) {
        Ok(t) => t,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to generate token: {e}")
            })));
        }
    };

    Ok(HttpResponse::Ok().json(LoginResponse {
        user: PublicUser::from(user),
        created,
        token,
    }))
}
