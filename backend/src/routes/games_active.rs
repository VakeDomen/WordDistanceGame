// src/routes/games.rs
use actix_web::{HttpResponse, get, web};
use chrono::{Datelike, Local};
use serde::Serialize;

use crate::{
    auth::jwt::Claims,
    db::{get_connection, types::DbError},
};

#[derive(Serialize)]
pub struct ActiveGamesResponse(pub Vec<Option<i64>>);

#[get("/games/active")]
pub async fn get_active_games(claims: web::ReqData<Claims>) -> actix_web::Result<HttpResponse> {
    let now = Local::now();
    let iso = now.iso_week();
    let week_code: i64 = (iso.year() as i64) * 100 + (iso.week() as i64);

    let data = match user_week_progress(&claims.sub, week_code) {
        Ok(v) => v,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to load progress: {e}")
            })));
        }
    };

    Ok(HttpResponse::Ok().json(ActiveGamesResponse(data)))
}

fn user_week_progress(user_id: &str, week_code: i64) -> Result<Vec<Option<i64>>, DbError> {
    use rusqlite::params;
    use std::collections::HashMap;

    let conn = get_connection()?;

    let mut stmt = conn.prepare(
        "SELECT id, game_seq_num, completed
         FROM games
         WHERE user_id = ?1 AND week = ?2",
    )?;
    let mut rows = stmt.query(params![user_id, week_code])?;

    let mut by_seq: HashMap<i64, (String, bool)> = HashMap::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let seq: i64 = row.get(1)?;
        let completed: i64 = row.get(2)?;
        by_seq.insert(seq, (id, completed != 0));
    }

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
