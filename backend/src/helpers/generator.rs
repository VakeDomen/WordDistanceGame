use chrono::{Datelike, Local};
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use rusqlite::params;
use std::{env, fs, path::Path, thread, time::Duration};

use crate::db::{get_connection, types::DbError};
use crate::models::target_word::{NewTargetWord, SqliteTargetWord};

pub fn spawn_weekly_generator() {
    thread::spawn(|| {
        loop {
            if let Err(e) = ensure_current_week_words() {
                eprintln!("[weekly-generator] error: {e}");
            }
            thread::sleep(Duration::from_secs(300));
        }
    });
}

fn ensure_current_week_words() -> Result<(), DbError> {
    let now = Local::now();
    let iso = now.iso_week();
    let week_code: i64 = (iso.year() as i64) * 100 + (iso.week() as i64);
    ensure_week_words_blocking(week_code)
}

fn ensure_week_words_blocking(week_code: i64) -> Result<(), DbError> {
    let conn = get_connection()?;

    let existing: i64 = conn.query_row(
        "SELECT COUNT(1) FROM target_words WHERE week = ?1",
        [week_code],
        |r| r.get(0),
    )?;
    if existing >= 100 {
        return Ok(());
    }

    let content = read_wordlist()?;
    let mut all_words: Vec<String> = content
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if all_words.len() < 100 {
        return Err(DbError::Other(
            "word list must have at least 100 words".into(),
        ));
    }

    let mut rng = StdRng::seed_from_u64(week_code as u64);
    all_words.shuffle(&mut rng);
    let selected: Vec<String> = all_words.into_iter().take(100).collect();

    // build SqliteTargetWord records; each TryFrom does one blocking HTTP call
    let mut to_insert = Vec::with_capacity(100);
    for w in selected {
        let new = NewTargetWord {
            week: week_code,
            word: w,
        };

        let mut stw = None;
        let mut attempts = 0;
        while stw.is_none() && attempts < 5 {
            stw = SqliteTargetWord::try_from(new.clone()).ok();
            attempts += 1;
        }
        if stw.is_none() {
            return Err(DbError::Other(
                "Can't initialize vector. Ollama problem.".into(),
            ));
        }
        to_insert.push(stw.unwrap());
    }

    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO target_words (id, week, word, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for stw in to_insert {
            let created_at = stw
                .created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let updated_at = stw
                .updated_at
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            stmt.execute(params![
                stw.id.to_string(),
                stw.week,
                stw.word,
                stw.embedding_json,
                created_at,
                updated_at
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn read_wordlist() -> Result<String, DbError> {
    let candidates = [
        env::var("WORDLIST_PATH").unwrap_or_default(),
        "backend/resources/cleaned_wordlist.txt".to_string(),
        "resources/cleaned_wordlist.txt".to_string(),
    ];
    for p in candidates {
        if p.is_empty() {
            continue;
        }
        let path = Path::new(&p);
        if path.exists() {
            return Ok(fs::read_to_string(path)?);
        }
    }
    Err(DbError::Other("word list not found".into()))
}
