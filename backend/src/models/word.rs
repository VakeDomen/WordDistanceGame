use rusqlite::Row;

use crate::{
    db::{get_connection, types::DbError},
    models::embedding::Embedding,
};

pub type WordId = String;

#[derive(Debug, Clone)]
pub struct Word {
    pub id: WordId,
    pub word: String,
    pub embeddings: Vec<Embedding>,
}

fn map_row(row: &Row) -> Result<Word, rusqlite::Error> {
    Ok(Word {
        id: row.get("id")?,
        word: row.get("word")?,
        embeddings: vec![],
    })
}

impl Word {
    pub fn get_by_value(w: &str) -> Result<Word, DbError> {
        let conn = get_connection()?;
        let mut stmt = conn.prepare("SELECT id, word FROM word_list WHERE word = ?1")?;
        let mut rows = stmt.query([w])?;
        let Some(row) = rows.next()? else {
            return Err(DbError::NotFound);
        };
        let mut word = map_row(row)?;
        word.embeddings = Embedding::for_word(&word.id)?;
        Ok(word)
    }

    pub fn get_by_id(id: &WordId) -> Result<Word, DbError> {
        let conn = get_connection()?;
        let mut stmt = conn.prepare("SELECT id, word FROM word_list WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;
        let Some(row) = rows.next()? else {
            return Err(DbError::NotFound);
        };
        let mut word = map_row(row)?;
        word.embeddings = Embedding::for_word(&word.id)?;
        Ok(word)
    }

    pub fn all_ids() -> Result<Vec<String>, DbError> {
        let conn = get_connection()?;
        let mut v = Vec::new();
        let mut stmt = conn.prepare("SELECT id FROM word_list")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            v.push(r.get(0)?);
        }
        Ok(v)
    }
}
