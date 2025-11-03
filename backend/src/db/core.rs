use crate::db::types::{Conn, DbError, DbPooled};
use once_cell::sync::OnceCell;
use r2d2::Pool;
use r2d2_sqlite::{SqliteConnectionManager, rusqlite::OpenFlags};
use std::{env, fs, path::Path, time::Duration};

static POOL: OnceCell<Pool<SqliteConnectionManager>> = OnceCell::new();

pub fn init_database() -> Result<(), DbError> {
    let db_path = env::var("SQLITE_DATABASE_PATH")?;

    if let Some(parent) = Path::new(&db_path).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_FULL_MUTEX;

    // Set per-connection PRAGMAs via the manager initializer
    let manager = SqliteConnectionManager::file(db_path)
        .with_flags(flags)
        .with_init(|conn| {
            conn.busy_timeout(Duration::from_secs(5))?;
            conn.pragma_update(None, "journal_mode", &"WAL")?;
            conn.pragma_update(None, "foreign_keys", &"ON")?;
            Ok(())
        });

    let pool = Pool::builder()
        .max_size(16)
        .min_idle(Some(2))
        .connection_timeout(Duration::from_secs(5))
        .build(manager)?;

    {
        let conn = pool.get()?;
        run_migrations(&conn)?;
    }

    POOL.set(pool).map_err(|_| DbError::AlreadyInitialized)?;
    Ok(())
}

pub fn get_connection() -> Result<DbPooled, DbError> {
    POOL.get()
        .ok_or(DbError::NotInitialized)?
        .get()
        .map_err(DbError::from)
}

fn run_migrations(conn: &Conn) -> Result<(), DbError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            ldap_id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE TRIGGER IF NOT EXISTS trg_users_updated_at
        AFTER UPDATE ON users
        FOR EACH ROW BEGIN
            UPDATE users SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = OLD.id;
        END;
        "#,
    )?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            week INTEGER NOT NULL,
            game_seq_num INTEGER NOT NULL CHECK (game_seq_num >= 0),
            completed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            FOREIGN KEY(user_id) REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_games_user_week_seq
            ON games(user_id, week, game_seq_num);
        CREATE INDEX IF NOT EXISTS idx_games_user_week
            ON games(user_id, week);
        CREATE TRIGGER IF NOT EXISTS trg_games_updated_at
        AFTER UPDATE ON games
        FOR EACH ROW BEGIN
            UPDATE games SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = OLD.id;
        END;
        "#,
    )?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS game_entries (
            id TEXT PRIMARY KEY,
            game_id TEXT NOT NULL,
            attempt_seq INTEGER NOT NULL CHECK (attempt_seq >= 1),
            value TEXT NOT NULL,
            dist INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            FOREIGN KEY(game_id) REFERENCES games(id) ON UPDATE CASCADE ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_game_entries_unique_attempt
            ON game_entries(game_id, attempt_seq);
        CREATE INDEX IF NOT EXISTS idx_game_entries_game
            ON game_entries(game_id);
        CREATE TRIGGER IF NOT EXISTS trg_game_entries_updated_at
        AFTER UPDATE ON game_entries
        FOR EACH ROW BEGIN
            UPDATE game_entries SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = OLD.id;
        END;
        "#
    )?;

    // target_words
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS target_words (
            id         TEXT PRIMARY KEY, -- uuid v4
            week       INTEGER NOT NULL,
            word_id    TEXT NOT NULL,    -- fk to word_list(id)
            seq        INTEGER NOT NULL CHECK (seq BETWEEN 0 AND 99),
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            UNIQUE(week, seq),
            UNIQUE(week, word_id),
            FOREIGN KEY(word_id) REFERENCES word_list(id) ON DELETE CASCADE ON UPDATE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_target_words_week ON target_words(week);
        CREATE TRIGGER IF NOT EXISTS trg_target_words_updated_at
        AFTER UPDATE ON target_words
        FOR EACH ROW BEGIN
            UPDATE target_words SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = OLD.id;
        END;
        "#,
    )?;

    conn.execute_batch(
        r#"
    CREATE TABLE IF NOT EXISTS word_list (
        id   TEXT PRIMARY KEY,       -- uuid v4
        word TEXT NOT NULL UNIQUE
    );

    CREATE TABLE IF NOT EXISTS embedding (
        id         TEXT PRIMARY KEY, -- uuid v4
        word_id    TEXT NOT NULL,    -- fk to word_list(id)
        text       TEXT NOT NULL,    -- the meaning/definition text you embedded
        embedding  TEXT NOT NULL,    -- JSON string: [f32, f32, ...]
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        FOREIGN KEY(word_id) REFERENCES word_list(id) ON DELETE CASCADE ON UPDATE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_embedding_word ON embedding(word_id);
    "#,
    )?;

    Ok(())
}
