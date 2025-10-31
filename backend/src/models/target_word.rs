use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::types::{Conn, DbError};

#[derive(Debug, Clone)]
pub struct TargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word: String,
    pub active: bool,
    pub embedding: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicTargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct NewTargetWord {
    pub week: i64,
    pub word: String,
    pub active: bool,
    pub embedding: Option<Vec<u8>>,
}

impl From<TargetWord> for PublicTargetWord {
    fn from(t: TargetWord) -> Self {
        Self {
            id: t.id,
            week: t.week,
            word: t.word,
            active: t.active,
        }
    }
}

fn map_row(row: &Row) -> Result<TargetWord, rusqlite::Error> {
    Ok(TargetWord {
        id: Uuid::parse_str(&row.get::<_, String>("id")?).unwrap(),
        week: row.get("week")?,
        word: row.get("word")?,
        active: row.get::<_, i64>("active")? != 0,
        embedding: row.get::<_, Option<Vec<u8>>>("embedding")?,
        created_at: row.get::<_, String>("created_at")?.parse().unwrap(),
        updated_at: row.get::<_, String>("updated_at")?.parse().unwrap(),
    })
}

impl TargetWord {
    pub fn insert(conn: &Conn, new_word: NewTargetWord) -> Result<TargetWord, DbError> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        conn.execute(
            "INSERT INTO target_words (id, week, word, active, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.to_string(),
                new_word.week,
                new_word.word,
                new_word.active as i64,
                new_word.embedding,
                now,
                now
            ],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Conn, id: &Uuid) -> Result<TargetWord, DbError> {
        let tw = conn.query_row(
            "SELECT id, week, word, active, embedding, created_at, updated_at
             FROM target_words WHERE id = ?1",
            [id.to_string()],
            map_row,
        )?;
        Ok(tw)
    }

    pub fn get_active_for_week(conn: &Conn, week: i64) -> Result<Vec<TargetWord>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, week, word, active, embedding, created_at, updated_at
             FROM target_words WHERE week = ?1 AND active = 1",
        )?;
        let iter = stmt.query_map([week], map_row)?;
        let mut out = Vec::new();
        for row in iter {
            out.push(row?);
        }
        Ok(out)
    }
}
