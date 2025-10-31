use actix_web::{App, HttpServer, web};
use actix_web_httpauth::middleware::HttpAuthentication;

use crate::{
    auth::jwt::validator,
    db::init_database,
    routes::{
        echo::echo,
        login::{login_employee, login_student},
    },
};

mod auth;
mod db;
mod models;
mod routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenv::dotenv();

    if let Err(e) = init_database() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            e,
        ));
    };

    let port = std::env::var("PORT").expect("PORT not set");
    let Ok(port) = port.parse() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "Invalid port",
        ));
    };

    HttpServer::new(|| {
        App::new()
            .service(login_employee)
            .service(login_student)
            .service(
                web::scope("/api")
                    .wrap(HttpAuthentication::bearer(validator))
                    .service(echo),
            )
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}
