#!/usr/bin/env python3
import json
import queue
import sqlite3
import sys
import time
import uuid
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import List, Optional, Tuple

import requests

# config
SQLITE_PATH = Path("../sqlite.db")
OLLAMA_URL = "http://hivecore.famnit.upr.si:6666/api/embed"
EMBED_MODEL = "hf.co/Qwen/Qwen3-Embedding-8B-GGUF:latest"
MAX_WORKERS = 10
RETRIES = 3
TIMEOUT = 30  # seconds
ERROR_LOG = Path("db_insertion_errors.jsonl")


def get_conn() -> sqlite3.Connection:
    # one connection per thread, autocommit mode
    conn = sqlite3.connect(
        str(SQLITE_PATH), timeout=30, isolation_level=None, check_same_thread=False
    )
    conn.execute("PRAGMA foreign_keys = ON;")
    # optional but nice
    conn.execute("PRAGMA journal_mode = WAL;")
    conn.execute("PRAGMA busy_timeout = 5000;")
    return conn


def ensure_word(conn: sqlite3.Connection, word: str) -> str:
    """
    Insert word into word_list if not exists; return its UUID string id.
    Assumes a UNIQUE constraint on word_list.word.
    """
    # try insert
    wid = str(uuid.uuid4())
    try:
        conn.execute(
            "INSERT OR IGNORE INTO word_list (id, word) VALUES (?, ?)",
            (wid, word),
        )
    except sqlite3.IntegrityError:
        pass

    # fetch id
    row = conn.execute("SELECT id FROM word_list WHERE word = ?", (word,)).fetchone()
    if not row:
        raise RuntimeError(f"failed to upsert word: {word!r}")
    return row[0]


def embed_texts_batch(texts: List[str]) -> List[List[float]]:
    """
    Calls Ollama once with all texts and returns a list of embedding vectors (floats).
    Retries on failure with exponential backoff.
    Accepts either {'data': [[...], ...]} or {'embeddings': [[...], ...]} shapes.
    """
    payload = {
        "model": EMBED_MODEL,
        "input": texts,
    }

    last_err = None
    for attempt in range(1, RETRIES + 1):
        try:
            resp = requests.post(OLLAMA_URL, json=payload, timeout=TIMEOUT)
            if resp.status_code != 200:
                last_err = RuntimeError(f"status {resp.status_code}: {resp.text[:500]}")
            else:
                j = resp.json()
                if "embeddings" in j and isinstance(j["embeddings"], list):
                    return j["embeddings"]
                if "data" in j and isinstance(j["data"], list):
                    return j["data"]
                last_err = RuntimeError(
                    "unexpected embedding JSON: missing 'embeddings' and 'data'"
                )
        except Exception as e:
            last_err = e

        # backoff
        if attempt < RETRIES:
            time.sleep(2 ** (attempt - 1))

    assert last_err is not None
    raise last_err


def insert_embeddings(
    conn: sqlite3.Connection,
    word_id: str,
    senses: List[str],
    vectors: List[List[float]],
) -> None:
    """
    Inserts each (sense text, embedding JSON) into embedding table.
    If a duplicate sense already exists for that word (unique constraint recommended on (word_id, text)),
    we skip it.
    """
    if len(senses) != len(vectors):
        raise ValueError(f"mismatch: {len(senses)} senses vs {len(vectors)} embeddings")

    # transaction
    conn.execute("BEGIN;")
    try:
        for text, vec in zip(senses, vectors):
            eid = str(uuid.uuid4())
            emb_json = json.dumps(vec, separators=(",", ":"))
            try:
                conn.execute(
                    "INSERT OR IGNORE INTO embedding (id, word_id, text, embedding, created_at) "
                    "VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                    (eid, word_id, text, emb_json),
                )
            except sqlite3.IntegrityError:
                # if no unique constraint exists, you might see duplicates on re-run.
                # you can switch to INSERT OR REPLACE or check existence first if desired.
                pass
        conn.execute("COMMIT;")
    except Exception:
        conn.execute("ROLLBACK;")
        raise


def process_jsonl_line(raw: str) -> Optional[dict]:
    """
    Returns None on success, or an error dict to be written to ERROR_LOG.
    """
    try:
        obj = json.loads(raw)
    except Exception as e:
        return {"error": f"invalid json: {e}", "line": raw[:500]}

    word = (obj.get("word") or "").strip()
    senses_en = obj.get("senses_en") or []
    if not word or not isinstance(senses_en, list) or len(senses_en) == 0:
        # skip per requirements
        return None

    # normalize and drop empties
    senses = [s.strip() for s in senses_en if isinstance(s, str) and s.strip()]
    if not senses:
        return None

    try:
        vecs = embed_texts_batch(senses)
    except Exception as e:
        return {"error": f"embedding_failed: {e}", "word": word, "senses": senses}

    try:
        conn = get_conn()
        with conn:
            wid = ensure_word(conn, word)
            insert_embeddings(conn, wid, senses, vecs)
        conn.close()
    except Exception as e:
        return {
            "error": f"db_insert_failed: {e}",
            "word": word,
            "senses": senses,
            "embeddings_count": len(vecs),
        }

    return None


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: python import_word_embeddings.py /path/to/file.jsonl")
        sys.exit(1)

    infile = Path(sys.argv[1])
    if not infile.exists():
        print(f"input file not found: {infile}")
        sys.exit(1)

    # prepare error log
    ERROR_LOG.parent.mkdir(parents=True, exist_ok=True)
    # clear old file
    if ERROR_LOG.exists():
        ERROR_LOG.unlink()

    # read all lines first (so we can submit tasks)
    with infile.open("r", encoding="utf-8") as f:
        lines = [ln for ln in f if ln.strip()]

    total = len(lines)
    print(f"processing {total} lines with {MAX_WORKERS} workers...")

    errors = 0

    # lock-free, append errors as they occur (open/close per write to avoid locking)
    def log_error(err: dict):
        nonlocal errors
        errors += 1
        with ERROR_LOG.open("a", encoding="utf-8") as ef:
            ef.write(json.dumps(err, ensure_ascii=False) + "\n")

    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as ex:
        futures = [ex.submit(process_jsonl_line, ln) for ln in lines]
        for i, fut in enumerate(as_completed(futures), start=1):
            try:
                err = fut.result()
                if err:
                    log_error(err)
            except Exception as e:
                log_error({"error": f"unhandled_exception: {e}"})
            if i % 50 == 0 or i == total:
                print(f"{i}/{total} done, errors so far: {errors}")

    print(f"done. total lines: {total}, errors: {errors}")
    if errors:
        print(f"see {ERROR_LOG}")


if __name__ == "__main__":
    main()
