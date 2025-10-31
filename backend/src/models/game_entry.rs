use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::types::{Conn, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub id: Uuid,
    pub game_id: Uuid,
    pub attempt_seq: i64,
    pub value: String,
    pub dist: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SqliteGameEntry {
    pub id: String,
    pub game_id: String,
    pub attempt_seq: i64,
    pub value: String,
    pub dist: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicGameEntry {
    pub id: Uuid,
    pub game_id: Uuid,
    pub attempt_seq: i64,
    pub value: String,
    pub dist: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGameEntry {
    pub game_id: Uuid,
    pub value: String,
    pub dist: i64,
}

impl From<SqliteGameEntry> for GameEntry {
    fn from(s: SqliteGameEntry) -> Self {
        GameEntry {
            id: Uuid::parse_str(&s.id).unwrap(),
            game_id: Uuid::parse_str(&s.game_id).unwrap(),
            attempt_seq: s.attempt_seq,
            value: s.value,
            dist: s.dist,
            created_at: s.created_at.parse().unwrap(),
            updated_at: s.updated_at.parse().unwrap(),
        }
    }
}

impl From<GameEntry> for PublicGameEntry {
    fn from(e: GameEntry) -> Self {
        PublicGameEntry {
            id: e.id,
            game_id: e.game_id,
            attempt_seq: e.attempt_seq,
            value: e.value,
            dist: e.dist,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }
}

impl From<NewGameEntry> for SqliteGameEntry {
    fn from(n: NewGameEntry) -> Self {
        SqliteGameEntry {
            id: Uuid::new_v4().to_string(),
            game_id: n.game_id.to_string(),
            attempt_seq: 0,
            value: n.value,
            dist: n.dist,
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            updated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

fn map_row(row: &Row) -> Result<SqliteGameEntry, rusqlite::Error> {
    Ok(SqliteGameEntry {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        attempt_seq: row.get("attempt_seq")?,
        value: row.get("value")?,
        dist: row.get("dist")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl GameEntry {
    pub fn insert(conn: &Conn, new_entry: NewGameEntry) -> Result<GameEntry, DbError> {
        let mut s: SqliteGameEntry = new_entry.into();

        // compute next attempt_seq per game
        let next_seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(attempt_seq) + 1, 1) FROM game_entries WHERE game_id = ?1",
            [s.game_id.clone()],
            |r| r.get(0),
        )?;
        s.attempt_seq = next_seq;

        conn.execute(
            "INSERT INTO game_entries (id, game_id, attempt_seq, value, dist, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![s.id, s.game_id, s.attempt_seq, s.value, s.dist, s.created_at, s.updated_at],
        )?;

        let e = conn.query_row(
            "SELECT id, game_id, attempt_seq, value, dist, created_at, updated_at
             FROM game_entries WHERE id = ?1",
            params![s.id],
            map_row,
        )?;
        Ok(GameEntry::from(e))
    }

    pub fn get_for_game(conn: &Conn, game_id: &Uuid) -> Result<Vec<GameEntry>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, attempt_seq, value, dist, created_at, updated_at
             FROM game_entries
             WHERE game_id = ?1
             ORDER BY attempt_seq ASC",
        )?;
        let rows = stmt.query_map(params![game_id.to_string()], map_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(GameEntry::from(r?));
        }
        Ok(out)
    }
}
