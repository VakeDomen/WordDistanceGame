use actix_web::{
    HttpResponse, cookie::time::format_description::well_known::iso8601::FormattedComponents, get,
    web,
};
use reqwest::redirect::Attempt;
use serde::Serialize;

use crate::{
    auth::jwt::Claims,
    db::get_connection,
    helpers::generator::get_week_code,
    models::{game::Game, game_entry::GameEntry},
};

#[derive(Debug, Serialize)]
pub struct Response {
    pub guesses: Vec<GuessOut>,
}

#[derive(Debug, Serialize)]
pub struct GuessOut {
    pub distance: i64,
    pub completed: bool,
    pub seq: i64,
    pub word: String,
}

impl From<Game> for Response {
    fn from(value: Game) -> Self {
        let guesses = value
            .attempts
            .iter()
            .map(|att| GuessOut::from((att, &value)))
            .collect();
        Self { guesses }
    }
}

impl From<(&GameEntry, &Game)> for GuessOut {
    fn from(value: (&GameEntry, &Game)) -> Self {
        Self {
            distance: value.0.dist,
            completed: value.1.completed,
            seq: value.0.attempt_seq,
            word: value.0.value.clone(),
        }
    }
}

#[get("/games/guess/{game_seq}")]
pub async fn get_attempts(
    claims: web::ReqData<Claims>,
    path: web::Path<i64>,
) -> actix_web::Result<HttpResponse> {
    let user_id = &claims.sub;
    let game_seq = *path;

    if !(1..101).contains(&game_seq) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "game_seq must be in [1, 100]"
        })));
    }

    let conn = match get_connection() {
        Ok(c) => c,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("db connection error: {e}")
            })));
        }
    };

    let week = get_week_code();
    let Ok(game) = Game::get_by_week_and_seq(week, game_seq) else {
        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("db connection error: could not fetch game")
        })));
    };

    Ok(HttpResponse::Ok().body(serde_json::to_string(&Response::from(game))?))
}
