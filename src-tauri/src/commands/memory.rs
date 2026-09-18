// Persistent memory for Hanna, backed by SQLite on disk (point this at
// your 1TB drive by setting `app_dir` to a path on it — see main.rs).
//
// Three tables:
//   messages     — raw conversation log, embedded for recall
//   facts        — durable facts *distilled* out of conversations
//                  (not raw transcript — see personality.rs on the
//                  frontend for the distillation step)
//   personality  — small key/value store of traits that drift slowly
//                  over time (tone, running jokes, things Hanna has
//                  decided about how she talks to Carlo)
//
// Recall is brute-force cosine similarity over stored embedding BLOBs.
// This is intentionally simple: a personal assistant's memory is
// thousands of rows, not millions, so there's no need for a real vector
// index (sqlite-vec, etc.) until it's actually slow. Swap it in later if
// so — the schema doesn't need to change, just `recall_similar`.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn db_path(app_dir: &str) -> PathBuf {
    PathBuf::from(app_dir).join("hanna_memory.sqlite3")
}

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[tauri::command]
pub async fn init_memory(app_dir: String) -> Result<(), String> {
    std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

    let conn = Connection::open(db_path(&app_dir)).map_err(|e| e.to_string())?;    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            embedding BLOB,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS facts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            embedding BLOB,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS personality (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;
    *DB.lock().unwrap() = Some(conn);
    Ok(())
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tauri::command]
pub async fn save_message(
    role: String,
    content: String,
    embedding: Vec<f32>,
) -> Result<(), String> {
    let guard = DB.lock().unwrap();
    let conn = guard.as_ref().ok_or("memory not initialized")?;
    conn.execute(
        "INSERT INTO messages (role, content, embedding, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![role, content, embedding_to_blob(&embedding), now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn save_fact(content: String, embedding: Vec<f32>) -> Result<(), String> {
    let guard = DB.lock().unwrap();
    let conn = guard.as_ref().ok_or("memory not initialized")?;
    conn.execute(
        "INSERT INTO facts (content, embedding, created_at) VALUES (?1, ?2, ?3)",
        params![content, embedding_to_blob(&embedding), now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct RecalledItem {
    pub content: String,
    pub score: f32,
}

/// Returns the top-k most relevant facts + past messages for a query
/// embedding, merged and ranked together by cosine similarity.
#[tauri::command]
pub async fn recall_similar(
    query_embedding: Vec<f32>,
    top_k: usize,
) -> Result<Vec<RecalledItem>, String> {
    let guard = DB.lock().unwrap();
    let conn = guard.as_ref().ok_or("memory not initialized")?;

    let mut results: Vec<RecalledItem> = Vec::new();

    let mut stmt = conn
        .prepare("SELECT content, embedding FROM facts")
        .map_err(|e| e.to_string())?;
    let fact_rows = stmt
        .query_map([], |row| {
            let content: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((content, blob))
        })
        .map_err(|e| e.to_string())?;
    for row in fact_rows.flatten() {
        let emb = blob_to_embedding(&row.1);
        let score = cosine_similarity(&query_embedding, &emb);
        results.push(RecalledItem { content: row.0, score });
    }

    // Recent conversation gets included too, so recall isn't only
    // "distilled facts" but also "things we literally just discussed".
    let mut stmt2 = conn
        .prepare("SELECT content, embedding FROM messages ORDER BY id DESC LIMIT 200")
        .map_err(|e| e.to_string())?;
    let msg_rows = stmt2
        .query_map([], |row| {
            let content: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((content, blob))
        })
        .map_err(|e| e.to_string())?;
    for row in msg_rows.flatten() {
        let emb = blob_to_embedding(&row.1);
        let score = cosine_similarity(&query_embedding, &emb);
        results.push(RecalledItem { content: row.0, score });
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    results.truncate(top_k);
    Ok(results)
}

#[tauri::command]
pub async fn get_personality() -> Result<Vec<(String, String)>, String> {
    let guard = DB.lock().unwrap();
    let conn = guard.as_ref().ok_or("memory not initialized")?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM personality")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub async fn update_personality(key: String, value: String) -> Result<(), String> {
    let guard = DB.lock().unwrap();
    let conn = guard.as_ref().ok_or("memory not initialized")?;
    conn.execute(
        "INSERT INTO personality (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
