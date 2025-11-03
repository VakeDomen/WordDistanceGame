use chrono::{DateTime, Utc};
use rusqlite::{Row, params};
use uuid::Uuid;

use crate::{
    db::{
        get_connection,
        types::{Conn, DbError},
    },
    helpers::generator::get_week_code,
    models::word::{Word, WordId},
};

#[derive(Debug, Clone)]
pub struct TargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word: Word,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewTargetWord {
    pub week: i64,
    pub word_id: WordId,
}

#[derive(Debug, Clone)]
pub struct SqliteTargetWord {
    pub id: Uuid,
    pub week: i64,
    pub word_id: WordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<NewTargetWord> for SqliteTargetWord {
    fn from(n: NewTargetWord) -> Self {
        let id = Uuid::new_v4();
        let now = Utc::now();

        Self {
            id,
            week: n.week,
            word_id: n.word_id,
            created_at: now,
            updated_at: now,
        }
    }
}

impl TryFrom<SqliteTargetWord> for TargetWord {
    type Error = DbError;

    fn try_from(s: SqliteTargetWord) -> Result<Self, DbError> {
        Ok(Self {
            id: s.id,
            week: s.week,
            word: Word::get_by_id(&s.word_id)?,
            created_at: s.created_at,
            updated_at: s.updated_at,
        })
    }
}

fn map_row_sqlite(row: &Row) -> Result<SqliteTargetWord, rusqlite::Error> {
    Ok(SqliteTargetWord {
        id: Uuid::parse_str(&row.get::<_, String>("id")?).unwrap(),
        week: row.get("week")?,
        word_id: row.get("word_id")?,
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
            "SELECT id, week, word_id, active, created_at, updated_at
             FROM target_words WHERE id = ?1",
            [id.to_string()],
            map_row_sqlite,
        )?;
        TargetWord::try_from(s)
    }

    pub fn get_active_for_week(conn: &Conn, week: i64) -> Result<Vec<TargetWord>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, week, word_id, created_at, updated_at
             FROM target_words WHERE week = ?1
             ORDER BY id",
        )?;
        let iter = stmt.query_map([week], map_row_sqlite)?;
        let mut out = Vec::new();
        for row in iter {
            out.push(TargetWord::try_from(row?)?);
        }
        Ok(out)
    }

    pub fn get_word_by_seq_current_week(seq: &i64) -> Result<TargetWord, DbError> {
        let conn = get_connection()?;
        let week_code = get_week_code();

        println!("Week: {week_code} {seq}");
        let mut stmt = conn.prepare(
            "SELECT id, week, word_id, created_at, updated_at
             FROM target_words WHERE week = ?1 AND seq = ?2
             ORDER BY seq",
        )?;
        let mut iter = stmt.query_map([week_code, *seq], map_row_sqlite)?;
        let Some(row_result) = iter.next() else {
            return Err(DbError::NotFound);
        };

        let Ok(row) = row_result else {
            return Err(DbError::NotFound);
        };

        TargetWord::try_from(row)
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
            "INSERT INTO target_words (id, week, word_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id.to_string(),
                self.week,
                self.word_id,
                created_at,
                updated_at
            ],
        )?;

        TargetWord::get_by_id(conn, &self.id)
    }
}
