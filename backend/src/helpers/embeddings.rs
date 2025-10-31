use std::env;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use crate::db::types::DbError;

#[derive(Deserialize)]
struct Resp {
    data: Option<Vec<Vec<f32>>>,
    embeddings: Option<Vec<Vec<f32>>>,
}

pub fn fetch_single_embedding_json_blocking(word: &str) -> Result<String, DbError> {
    let Ok(url) = env::var("OLLAMA_EMBEDDING_ENDPOINT") else {
        return Err(DbError::Other(format!("Ollama endpoint not defined")));
    };

    let body = json!({
        "model": "bge-m3",
        "input": [word],
    });

    let resp = Client::new()
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| DbError::Other(format!("embedding request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().unwrap_or_default();
        return Err(DbError::Other(format!(
            "embedding server {}: {}",
            status, txt
        )));
    }

    let parsed: Resp = resp
        .json()
        .map_err(|e| DbError::Other(format!("embedding parse failed: {e}")))?;

    let vecf = parsed
        .embeddings
        .or(parsed.data)
        .and_then(|mut v| v.pop()) // exactly one word requested
        .ok_or_else(|| DbError::Other("embedding missing in response".to_string()))?;

    serde_json::to_string(&vecf)
        .map_err(|e| DbError::Other(format!("embedding serialize failed: {e}")))
}
