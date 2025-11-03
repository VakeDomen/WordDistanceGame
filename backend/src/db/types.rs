use std::{env, error::Error, fmt};

use r2d2::PooledConnection;
use r2d2_sqlite::{SqliteConnectionManager, rusqlite};

pub type DbPooled = PooledConnection<SqliteConnectionManager>;
pub type Conn = rusqlite::Connection;

/// Basic database error type
#[derive(Debug)]
pub enum DbError {
    MissingEnv(env::VarError),
    Io(std::io::Error),
    Sql(rusqlite::Error),
    Pool(r2d2::Error),
    AlreadyInitialized,
    NotInitialized,
    Other(String),
    NotFound,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::MissingEnv(e) => write!(f, "missing env var: {e}"),
            DbError::Io(e) => write!(f, "io error: {e}"),
            DbError::Sql(e) => write!(f, "sqlite error: {e}"),
            DbError::Pool(e) => write!(f, "pool error: {e}"),
            DbError::AlreadyInitialized => write!(f, "database already initialized"),
            DbError::NotInitialized => write!(f, "database not initialized"),
            DbError::Other(s) => write!(f, "{s}"),
            DbError::NotFound => write!(f, "No value found"),
        }
    }
}

impl Error for DbError {}

impl From<env::VarError> for DbError {
    fn from(e: env::VarError) -> Self {
        DbError::MissingEnv(e)
    }
}

impl From<std::io::Error> for DbError {
    fn from(e: std::io::Error) -> Self {
        DbError::Io(e)
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sql(e)
    }
}

impl From<r2d2::Error> for DbError {
    fn from(e: r2d2::Error) -> Self {
        DbError::Pool(e)
    }
}
