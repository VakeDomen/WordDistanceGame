use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::{
        get_connection,
        types::{Conn, DbError},
    },
    models::game_entry::GameEntry,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: Uuid,
    pub user_id: Uuid,
    pub week: i64,
    pub game_seq_num: i64,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub attempts: Vec<GameEntry>,
}

#[derive(Debug, Clone)]
pub struct SqliteGame {
    pub id: String,
    pub user_id: String,
    pub week: i64,
    pub game_seq_num: i64,
    pub completed: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicGame {
    pub id: Uuid,
    pub user_id: Uuid,
    pub week: i64,
    pub game_seq_num: i64,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGame {
    pub user_id: Uuid,
    pub week: i64,
    pub game_seq_num: i64,
    pub completed: bool,
}

impl TryFrom<SqliteGame> for Game {
    type Error = DbError;

    fn try_from(s: SqliteGame) -> Result<Self, DbError> {
        let id = Uuid::parse_str(&s.id).unwrap();
        let attempts = GameEntry::get_for_game(&id)?;

        Ok(Self {
            id,
            user_id: Uuid::parse_str(&s.user_id).unwrap(),
            week: s.week,
            game_seq_num: s.game_seq_num,
            completed: s.completed != 0,
            created_at: s.created_at.parse().unwrap(),
            updated_at: s.updated_at.parse().unwrap(),
            attempts,
        })
    }
}

impl From<Game> for PublicGame {
    fn from(g: Game) -> Self {
        PublicGame {
            id: g.id,
            user_id: g.user_id,
            week: g.week,
            game_seq_num: g.game_seq_num,
            completed: g.completed,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}

impl From<NewGame> for SqliteGame {
    fn from(n: NewGame) -> Self {
        SqliteGame {
            id: Uuid::new_v4().to_string(),
            user_id: n.user_id.to_string(),
            week: n.week,
            game_seq_num: n.game_seq_num,
            completed: if n.completed { 1 } else { 0 },
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            updated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

fn map_row(row: &Row) -> Result<SqliteGame, rusqlite::Error> {
    Ok(SqliteGame {
        id: row.get("id")?,
        user_id: row.get("user_id")?,
        week: row.get("week")?,
        game_seq_num: row.get("game_seq_num")?,
        completed: row.get("completed")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl Game {
    pub fn insert(conn: &Conn, new_game: NewGame) -> Result<Game, DbError> {
        let s: SqliteGame = new_game.into();
        conn.execute(
            "INSERT INTO games (id, user_id, week, game_seq_num, completed, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                s.id,
                s.user_id,
                s.week,
                s.game_seq_num,
                s.completed,
                s.created_at,
                s.updated_at
            ],
        )?;
        let g = Self::get_by_id(conn, &s.id)?;
        Ok(g)
    }

    pub fn get_by_id(conn: &Conn, id_str: &str) -> Result<Game, DbError> {
        let s: SqliteGame = conn.query_row(
            "SELECT id, user_id, week, game_seq_num, completed, created_at, updated_at
             FROM games WHERE id = ?1",
            params![id_str],
            map_row,
        )?;
        Ok(Game::try_from(s)?)
    }

    pub fn get_by_week_and_seq(week: i64, seq: i64) -> Result<Game, DbError> {
        let conn = get_connection()?;
        let s: SqliteGame = conn.query_row(
            "SELECT id, user_id, week, game_seq_num, completed, created_at, updated_at
             FROM games WHERE week = ?1 AND game_seq_num = ?2",
            params![week, seq],
            map_row,
        )?;
        Ok(Game::try_from(s)?)
    }

    pub fn get_user_games(conn: &Conn, user_id: &Uuid) -> Result<Vec<Game>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, week, game_seq_num, completed, created_at, updated_at
             FROM games
             WHERE user_id = ?1
             ORDER BY week ASC, game_seq_num ASC",
        )?;
        let rows = stmt.query_map(params![user_id.to_string()], map_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(Game::try_from(r?)?);
        }
        Ok(out)
    }

    pub fn set_completed(conn: &Conn, game_id: &Uuid, completed: bool) -> Result<(), DbError> {
        conn.execute(
            "UPDATE games SET completed = ?1 WHERE id = ?2",
            params![i64::from(completed as i32), game_id.to_string()],
        )?;
        Ok(())
    }
}
