use std::env;

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    db::types::{Conn, DbError},
    helpers::embeddings::fetch_single_embedding_json_blocking,
};

#[derive(Debug, Clone)]
pub struct TargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word: String,
    pub embedding: Vec<f32>, // parsed on read
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicTargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word: String,
}

#[derive(Debug, Clone)]
pub struct NewTargetWord {
    pub week: i64,
    pub word: String,
}

#[derive(Debug, Clone)]
pub struct SqliteTargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word: String,
    pub embedding_json: Option<String>, // stored as TEXT
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TargetWord> for PublicTargetWord {
    fn from(t: TargetWord) -> Self {
        Self {
            id: t.id,
            week: t.week,
            word: t.word,
        }
    }
}

impl TryFrom<NewTargetWord> for SqliteTargetWord {
    type Error = DbError;

    fn try_from(n: NewTargetWord) -> Result<Self, Self::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        // one word = one request, embedding saved as JSON string
        let embedding_json = Some(fetch_single_embedding_json_blocking(&n.word)?);

        Ok(Self {
            id,
            week: n.week,
            word: n.word,
            embedding_json,
            created_at: now,
            updated_at: now,
        })
    }
}

impl From<SqliteTargetWord> for TargetWord {
    fn from(s: SqliteTargetWord) -> Self {
        let embedding = s
            .embedding_json
            .as_deref()
            .and_then(|j| serde_json::from_str::<Vec<f32>>(j).ok())
            .unwrap_or_default();

        Self {
            id: s.id,
            week: s.week,
            word: s.word,
            embedding,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

fn map_row_sqlite(row: &Row) -> Result<SqliteTargetWord, rusqlite::Error> {
    Ok(SqliteTargetWord {
        id: Uuid::parse_str(&row.get::<_, String>("id")?).unwrap(),
        week: row.get("week")?,
        word: row.get("word")?,
        embedding_json: row.get::<_, Option<String>>("embedding")?,
        created_at: row
            .get::<_, String>("created_at")?
            .parse()
            .expect("invalid created_at rfc3339"),
        updated_at: row
            .get::<_, String>("updated_at")?
            .parse()
            .expect("invalid updated_at rfc3339"),
    })
}

impl TargetWord {
    pub fn get_by_id(conn: &Conn, id: &Uuid) -> Result<TargetWord, DbError> {
        let s = conn.query_row(
            "SELECT id, week, word, active, embedding, created_at, updated_at
             FROM target_words WHERE id = ?1",
            [id.to_string()],
            map_row_sqlite,
        )?;
        Ok(TargetWord::from(s))
    }

    pub fn get_active_for_week(conn: &Conn, week: i64) -> Result<Vec<TargetWord>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, week, word, embedding, created_at, updated_at
             FROM target_words WHERE week = ?1
             ORDER BY id",
        )?;
        let iter = stmt.query_map([week], map_row_sqlite)?;
        let mut out = Vec::new();
        for row in iter {
            out.push(TargetWord::from(row?));
        }
        Ok(out)
    }
}

impl SqliteTargetWord {
    pub fn insert(&self, conn: &Conn) -> Result<TargetWord, DbError> {
        let created_at = self
            .created_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let updated_at = self
            .updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        conn.execute(
            "INSERT INTO target_words (id, week, word, active, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id.to_string(),
                self.week,
                self.word,
                self.embedding_json,
                created_at,
                updated_at
            ],
        )?;

        TargetWord::get_by_id(conn, &self.id)
    }
}
