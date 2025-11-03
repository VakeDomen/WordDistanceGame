use chrono::{Datelike, Local};
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use rusqlite::params;
use std::{thread, time::Duration};

use crate::db::{get_connection, types::DbError};
use crate::models::target_word::{NewTargetWord, SqliteTargetWord};
use crate::models::word::Word;

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

    let mut all_word_ids = Word::all_ids()?;

    if all_word_ids.len() < 100 {
        return Err(DbError::Other(
            "word list must have at least 100 words".into(),
        ));
    }

    let mut rng = StdRng::seed_from_u64(week_code as u64);
    all_word_ids.shuffle(&mut rng);
    let selected: Vec<String> = all_word_ids.into_iter().take(100).collect();

    let mut to_insert = Vec::with_capacity(100);
    for w in selected {
        let new = NewTargetWord {
            week: week_code,
            word_id: w,
        };

        to_insert.push(SqliteTargetWord::from(new.clone()));
    }

    let tx = conn.unchecked_transaction()?;
    let mut inserted = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO target_words
                     (id, week, word_id, seq, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4,
                             strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                             strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        )?;

        for stw in to_insert {
            // adjust to_string calls if word_id is a Uuid
            let changed = stmt.execute(params![
                stw.id.to_string(),
                stw.week,
                stw.word_id.to_string(),
                inserted,
            ])?;
            inserted += changed as usize;
        }
    }
    tx.commit()?;
    println!("[weekly generator] Generated words for the new week");
    Ok(())
}

pub fn get_week_code() -> i64 {
    let now = Local::now();
    let iso = now.iso_week();
    (iso.year() as i64) * 100 + (iso.week() as i64)
}
