use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::types::{Conn, DbError};

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub ldap_id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SqliteUser {
    pub id: String,
    pub ldap_id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicUser {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub ldap_id: String,
    pub name: String,
}

impl From<SqliteUser> for User {
    fn from(s: SqliteUser) -> Self {
        User {
            id: Uuid::parse_str(&s.id).expect("valid uuid"),
            ldap_id: s.ldap_id,
            name: s.name,
            created_at: s.created_at.parse().expect("valid timestamp"),
            updated_at: s.updated_at.parse().expect("valid timestamp"),
        }
    }
}

impl From<User> for PublicUser {
    fn from(u: User) -> Self {
        PublicUser {
            id: u.id,
            name: u.name,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

impl From<NewUser> for SqliteUser {
    fn from(n: NewUser) -> Self {
        SqliteUser {
            id: Uuid::new_v4().to_string(),
            ldap_id: n.ldap_id,
            name: n.name,
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            updated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

fn map_row(row: &Row) -> Result<SqliteUser, rusqlite::Error> {
    Ok(SqliteUser {
        id: row.get("id")?,
        ldap_id: row.get("ldap_id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl User {
    pub fn insert(conn: &Conn, new_user: NewUser) -> Result<User, DbError> {
        let s: SqliteUser = new_user.into();
        conn.execute(
            "INSERT INTO users (id, ldap_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![s.id, s.ldap_id, s.name, s.created_at, s.updated_at],
        )?;
        // fetch roundtrip to normalize timestamps
        let fetched = Self::get_by_id(conn, &s.id)?;
        Ok(fetched)
    }

    pub fn get_by_id(conn: &Conn, id_str: &str) -> Result<User, DbError> {
        let s: SqliteUser = conn.query_row(
            "SELECT id, ldap_id, name, created_at, updated_at FROM users WHERE id = ?1",
            params![id_str],
            map_row,
        )?;
        Ok(User::from(s))
    }

    pub fn get_by_ldap(conn: &Conn, ldap_id: &str) -> Result<User, DbError> {
        let s: SqliteUser = conn.query_row(
            "SELECT id, ldap_id, name, created_at, updated_at FROM users WHERE ldap_id = ?1",
            params![ldap_id],
            map_row,
        )?;
        Ok(User::from(s))
    }
}
