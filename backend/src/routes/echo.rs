use actix_web::{HttpResponse, post, web};
use serde::{Deserialize, Serialize};

use crate::auth::jwt::Claims;

#[derive(Debug, Deserialize)]
pub struct EchoIn {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct EchoOut {
    pub user_id: String,
    pub message: String,
}

#[post("/echo")]
pub async fn echo(
    claims: web::ReqData<Claims>,
    body: web::Json<EchoIn>,
) -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(EchoOut {
        user_id: claims.sub.clone(),
        message: body.message.clone(),
    }))
}
