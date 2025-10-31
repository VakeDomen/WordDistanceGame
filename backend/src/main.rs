use actix_web::{App, HttpServer};

use crate::{
    db::init_database,
    routes::login::{login_employee, login_student},
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

    HttpServer::new(|| App::new().service(login_employee).service(login_student))
        .bind(("127.0.0.1", port))?
        .run()
        .await
}
