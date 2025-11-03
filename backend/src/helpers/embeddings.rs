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

    let positioned_word = ["Slovenska beseda: ", word].join("");
    let body = json!({
        "model": "hf.co/Qwen/Qwen3-Embedding-8B-GGUF",
        "input": [positioned_word],
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

pub async fn fetch_single_embedding_vec_async(word: &str) -> Result<Vec<f32>, String> {
    use reqwest::StatusCode;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct Resp {
        data: Option<Vec<Vec<f32>>>,
        embeddings: Option<Vec<Vec<f32>>>,
    }

    let url = env::var("OLLAMA_EMBEDDING_ENDPOINT")
        .unwrap_or_else(|_| "http://hivecore.famnit.upr.si:6666/api/embed".to_string());

    let client = reqwest::Client::new();
    let positioned_word = ["Slovenska beseda: ", word].join("");
    let body = json!({ "model": "hf.co/Qwen/Qwen3-Embedding-8B-GGUF", "input": [positioned_word] });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    if status != StatusCode::OK {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("embedding server status {}: {}", status, text));
    }
    let parsed: Resp = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .embeddings
        .or(parsed.data)
        .and_then(|mut v| v.pop())
        .ok_or_else(|| "embedding missing in response".to_string())
}
