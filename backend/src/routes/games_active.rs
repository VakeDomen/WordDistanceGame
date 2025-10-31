// src/routes/games.rs
use actix_web::{HttpResponse, get, web};
use chrono::{Datelike, Local};
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use serde::Serialize;
use std::{fs, path::Path};

use crate::{
    auth::jwt::Claims,
    db::{get_connection, types::DbError},
};

#[derive(Serialize)]
pub struct ActiveGamesResponse(pub Vec<Option<i64>>);

#[get("/games/active")]
pub async fn get_active_games(claims: web::ReqData<Claims>) -> actix_web::Result<HttpResponse> {
    // figure out this ISO week
    let now = Local::now();
    let iso = now.iso_week();
    let week_code: i64 = (iso.year() as i64) * 100 + (iso.week() as i64);

    // make sure weekly target words exist
    if let Err(e) = ensure_week_words(week_code) {
        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to prepare weekly words: {e}")
        })));
    }

    // build the 100-element progress vector for this user
    let user_id = &claims.sub;
    let data = match user_week_progress(user_id, week_code) {
        Ok(v) => v,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to load progress: {e}")
            })));
        }
    };

    Ok(HttpResponse::Ok().json(ActiveGamesResponse(data)))
}

/// Ensure target_words for week_code exist by sampling 100 unique words with a deterministic seed.
fn ensure_week_words(week_code: i64) -> Result<(), DbError> {
    let conn = get_connection()?;

    let existing: i64 = conn.query_row(
        "SELECT COUNT(1) FROM target_words WHERE week = ?1",
        [week_code],
        |r| r.get(0),
    )?;

    if existing > 0 {
        return Ok(());
    }

    // read word list from disk
    let path_candidates = [
        std::env::var("WORDLIST_PATH").unwrap_or_default(),
        "backend/resources/cleaned_wordlist.txt".to_string(),
        "resources/cleaned_wordlist.txt".to_string(),
    ];

    let mut content = None;
    for p in path_candidates {
        if p.is_empty() {
            continue;
        }
        let path = Path::new(&p);
        if path.exists() {
            content = Some(fs::read_to_string(path)?);
            break;
        }
    }

    let content = content.ok_or_else(|| DbError::Other("word list not found".into()))?;
    let mut words: Vec<String> = content
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if words.len() < 100 {
        return Err(DbError::Other(
            "word list must have at least 100 words".into(),
        ));
    }

    // deterministic sample of 100 using seed = week_code as u64
    let seed = week_code as u64;
    let mut rng = StdRng::seed_from_u64(seed);
    words.shuffle(&mut rng);
    let selected = &words[..100];

    // insert targets
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO target_words (id, week, word, active, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, NULL,
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        )?;
        for w in selected {
            let id = uuid::Uuid::new_v4().to_string();
            stmt.execute((id, week_code, w))?;
        }
    }
    tx.commit()?;

    Ok(())
}

/// Build a 100-element vector for the user and week:
/// if game is completed return number of attempts, otherwise null.
fn user_week_progress(user_id: &str, week_code: i64) -> Result<Vec<Option<i64>>, DbError> {
    use rusqlite::params;
    let conn = get_connection()?;

    // load all games for this user and week, keyed by seq number
    let mut stmt = conn.prepare(
        "SELECT id, game_seq_num, completed
         FROM games
         WHERE user_id = ?1 AND week = ?2",
    )?;
    let mut rows = stmt.query(params![user_id, week_code])?;

    use std::collections::HashMap;
    let mut by_seq: HashMap<i64, (String, bool)> = HashMap::new();

    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let seq: i64 = row.get(1)?;
        let completed: i64 = row.get(2)?; // 0 or 1
        by_seq.insert(seq, (id, completed != 0));
    }

    // Build response
    let mut out = vec![None; 100];
    for seq in 0..100_i64 {
        if let Some((game_id, completed)) = by_seq.get(&seq) {
            if *completed {
                let attempts: i64 = conn.query_row(
                    "SELECT COUNT(1) FROM game_entries WHERE game_id = ?1",
                    [game_id],
                    |r| r.get(0),
                )?;
                out[seq as usize] = Some(attempts);
            }
        }
    }

    Ok(out)
}
