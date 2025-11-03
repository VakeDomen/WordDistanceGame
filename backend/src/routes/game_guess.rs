use actix_web::{HttpResponse, post, web};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{
    auth::jwt::Claims,
    db::{get_connection, types::DbError},
    helpers::generator::get_week_code,
    models::{target_word::TargetWord, word::Word},
};

#[derive(Deserialize)]
pub struct GuessIn {
    pub guess: String,
}

#[derive(Serialize)]
pub struct GuessOut {
    pub distance: i64, // cosine distance * 1000, rounded
    pub completed: bool,
    pub attempts: i64,
}

#[post("/games/guess/{game_seq}")]
pub async fn guess_word(
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
    body: web::Json<GuessIn>,
) -> actix_web::Result<HttpResponse> {
    let user_id = &claims.sub;
    let game_seq = *path;

    if !(1..101).contains(&game_seq) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "game_seq must be in [0, 99]"
        })));
    }

    // current ISO week code: year*100 + week

    let conn = match get_connection() {
        Ok(c) => c,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("db connection error: {e}")
            })));
        }
    };

    // fetch the N-th word by a deterministic order
    let Ok(target_word) = TargetWord::get_word_by_seq_current_week(&game_seq) else {
        return Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Can not find game with given sequence number.")
        })));
    };

    // validate guess
    let guess_content = body.guess.trim();
    let Ok(guess) = Word::get_by_value(guess_content) else {
        return Ok(HttpResponse::NoContent().json(serde_json::json!({
            "error": format!("Can not find the guessed word in the dictionary")
        })));
    };

    // find or create the game row for this user-week-seq
    let week_code = get_week_code();
    let game_id: String = match conn.query_row::<String, _, _>(
        "SELECT id FROM games WHERE user_id = ?1 AND week = ?2 AND game_seq_num = ?3",
        params![user_id, week_code, game_seq],
        |r| r.get(0),
    ) {
        Ok(id) => id,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let id = uuid::Uuid::new_v4().to_string();
            if let Err(e) = conn.execute(
                "INSERT INTO games (id, user_id, week, game_seq_num, completed, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0,
                         strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                         strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                params![id, user_id, week_code, game_seq],
            ) {
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("failed to create game: {e}")
                })));
            }
            id
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to read game: {e}")
            })));
        }
    };

    // already completed
    let completed: i64 = match conn.query_row(
        "SELECT completed FROM games WHERE id = ?1",
        [game_id.as_str()],
        |r| r.get(0),
    ) {
        Ok(v) => v,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to read game status: {e}")
            })));
        }
    };
    if completed != 0 {
        let attempts = count_attempts(&conn, &game_id).unwrap_or(0);
        return Ok(HttpResponse::Conflict().json(GuessOut {
            distance: 0,
            completed: true,
            attempts,
        }));
    }

    // compute cosine distance scaled by 1000 to an integer
    let mut shortest = i64::MAX;
    for guess_embedding in &guess.embeddings {
        for target_embedding in &target_word.word.embeddings {
            let dist1000 = cosine_distance_thousandths(
                &guess_embedding.embedding,
                &target_embedding.embedding,
            );
            if dist1000 < shortest {
                shortest = dist1000;
            }
        }
    }

    // next attempt_seq
    let attempt_seq: i64 = next_attempt_seq(&conn, &game_id).unwrap_or(1);

    // insert attempt
    let entry_id = uuid::Uuid::new_v4().to_string();
    println!("Guess: {:#?} | ({:#?})", guess, target_word.word);
    if let Err(e) = conn.execute(
        "INSERT INTO game_entries (id, game_id, attempt_seq, value, dist, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![entry_id, game_id, attempt_seq, guess.word, shortest],
    ) {
        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to insert attempt: {e}")
        })));
    }

    // mark completed if guessed exact word case-insensitively
    let equals = guess.word.eq_ignore_ascii_case(&target_word.word.word);
    if equals {
        if let Err(e) = conn.execute(
            "UPDATE games SET completed = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
            [game_id.as_str()],
        ) {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to complete game: {e}")
            })));
        }
    }

    let attempts = count_attempts(&conn, &game_id).unwrap_or(attempt_seq);
    Ok(HttpResponse::Ok().json(GuessOut {
        distance: shortest,
        completed: equals,
        attempts,
    }))
}

/* helpers from your existing file */

fn next_attempt_seq(conn: &rusqlite::Connection, game_id: &str) -> Result<i64, DbError> {
    let max_seq: Option<i64> = conn
        .query_row(
            "SELECT MAX(attempt_seq) FROM game_entries WHERE game_id = ?1",
            [game_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(DbError::from)?;
    Ok(max_seq.unwrap_or(0) + 1)
}

fn count_attempts(conn: &rusqlite::Connection, game_id: &str) -> Result<i64, DbError> {
    conn.query_row(
        "SELECT COUNT(1) FROM game_entries WHERE game_id = ?1",
        [game_id],
        |r| r.get(0),
    )
    .map_err(DbError::from)
}

fn cosine_distance_thousandths(a: &[f32], b: &[f32]) -> i64 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 1000;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        dot += ai * bi;
        na += ai * ai;
        nb += bi * bi;
    }
    if na == 0.0 || nb == 0.0 {
        return 1000;
    }
    let cos = (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0);
    let dist = 1.0 - cos;
    (dist * 1000.0).round() as i64
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> i64 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 1000;
    }
    let mut sum_sq = 0.0f64;
    for i in 0..a.len() {
        let diff = (a[i] as f64) - (b[i] as f64);
        sum_sq += diff * diff;
    }
    let dist = sum_sq.sqrt();
    // Map to 0-1000 scale: assume max distance is 2.0 (arbitrary but safe)
    let max_dist = 2.0;
    let normalized = (dist / max_dist).clamp(0.0, 1.0);
    (normalized * 1000.0).round() as i64
}
