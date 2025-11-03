use rusqlite::Row;
use serde::Serialize;

use crate::{
    db::{get_connection, types::DbError},
    models::word::WordId,
};

#[derive(Debug, Clone)]
pub struct Embedding {
    id: String,
    word_id: String,
    text: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SqliteEmbedding {
    id: String,
    word_id: String,
    text: String,
    embedding: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicEmbedding {
    id: String,
    word_id: String,
    text: String,
    embedding: Vec<f32>,
}

pub type EmbeddingId = String;

fn map_row(row: &Row) -> Result<SqliteEmbedding, rusqlite::Error> {
    Ok(SqliteEmbedding {
        id: row.get("id")?,
        word_id: row.get("word_id")?,
        text: row.get("text")?,
        embedding: row.get("embedding")?,
    })
}

impl Embedding {
    pub fn for_word(word_id: &WordId) -> Result<Vec<Embedding>, DbError> {
        let conn = get_connection()?;
        let mut stmt =
            conn.prepare("SELECT id, word_id, text, embedding FROM embedding WHERE word_id = ?1")?;
        let mut rows = stmt.query([word_id.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(Embedding::try_from(map_row(r)?)?);
        }
        Ok(out)
    }
}

impl TryFrom<EmbeddingId> for Embedding {
    type Error = DbError;
    fn try_from(value: EmbeddingId) -> Result<Self, DbError> {
        let conn = get_connection()?;
        let mut stmt =
            conn.prepare("SELECT id, word_id, text, embedding FROM embedding WHERE id = ?1")?;
        let mut rows = stmt.query([value])?;
        while let Some(r) = rows.next()? {
            return Embedding::try_from(map_row(r)?);
        }
        Err(DbError::NotFound)
    }
}

impl TryFrom<SqliteEmbedding> for Embedding {
    type Error = DbError;

    fn try_from(value: SqliteEmbedding) -> Result<Self, DbError> {
        let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&value.embedding) else {
            return Err(DbError::Other("Invalid embedding".into()));
        };

        Ok(Self {
            id: value.id,
            word_id: value.word_id,
            text: value.text,
            embedding,
        })
    }
}
