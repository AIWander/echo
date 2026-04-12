//! Semantic Search Module for Echo
//!
//! Embedding-based document search using Ollama + SQLite.
//! Model-agnostic: caller specifies embedding model per call.
//! Store metadata records which model+dimension was used at index time.
//! Mismatched model on search returns a clear error with rebuild hint.
//! Migrated from utonomous as part of INTAKE/APPLICATION architecture split.
// NAV: TOC at line 320 | 9 fn | 5 struct | 2026-04-11

use anyhow::{Result, Context};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

/// Ollama client for embedding operations
struct EmbedClient {
    client: reqwest::Client,
}

impl EmbedClient {
    fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn embed(&self, text: &str, model: &str) -> Result<Vec<f32>> {
        let resp = self.client
            .post(format!("{}/api/embeddings", OLLAMA_URL))
            .json(&EmbedRequest {
                model: model.to_string(),
                prompt: text.to_string(),
            })
            .send()
            .await
            .context("Failed to connect to Ollama")?
            .json::<EmbedResponse>()
            .await
            .context("Failed to parse Ollama response")?;
        Ok(resp.embedding)
    }
}

/// Semantic search result
#[derive(Debug, Clone)]
pub struct SemanticResult {
    pub path: String,
    pub title: String,
    pub content: String,
    pub similarity: f32,
}

/// SQLite-backed semantic search index
pub struct SemanticIndex {
    embed: EmbedClient,
    conn: Connection,
}

impl SemanticIndex {
    pub fn open_or_create(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS store_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )", [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                title TEXT,
                chunk_start INTEGER,
                chunk_end INTEGER,
                content TEXT
            )", [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS embeddings (
                doc_id INTEGER NOT NULL,
                dim INTEGER NOT NULL,
                value REAL NOT NULL,
                PRIMARY KEY (doc_id, dim),
                FOREIGN KEY (doc_id) REFERENCES documents(id)
            )", [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_doc ON embeddings(doc_id)", [],
        )?;
        Ok(Self { embed: EmbedClient::new(), conn })
    }

    pub fn set_metadata(&self, model: &str, dimension: usize) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO store_meta (key, value) VALUES ('embedding_model', ?1)",
            [model],
        )?;
        self.conn.execute(
            "INSERT OR REPLACE INTO store_meta (key, value) VALUES ('dimension', ?1)",
            [&dimension.to_string()],
        )?;
        Ok(())
    }

    pub fn get_metadata(&self) -> Result<Option<(String, usize)>> {
        let model: rusqlite::Result<String> = self.conn.query_row(
            "SELECT value FROM store_meta WHERE key = 'embedding_model'", [], |r| r.get(0)
        );
        let dim: rusqlite::Result<String> = self.conn.query_row(
            "SELECT value FROM store_meta WHERE key = 'dimension'", [], |r| r.get(0)
        );
        match (model, dim) {
            (Ok(m), Ok(d)) => Ok(Some((m, d.parse().unwrap_or(0)))),
            _ => Ok(None),
        }
    }

    pub async fn search(&self, query: &str, model: &str, limit: usize) -> Result<Vec<SemanticResult>> {
        // Verify model matches stored metadata
        match self.get_metadata()? {
            Some((stored_model, stored_dim)) => {
                if stored_model != model {
                    return Err(anyhow::anyhow!(
                        "Store was built with model '{}' (dim {}), you requested '{}'. Run semantic_reindex to rebuild.",
                        stored_model, stored_dim, model
                    ));
                }
            }
            None => {
                return Err(anyhow::anyhow!(
                    "Store has no metadata. Run semantic_reindex first."
                ));
            }
        }

        let query_vec = self.embed.embed(query, model).await?;
        let mut stmt = self.conn.prepare(
            "SELECT d.id, d.path, d.title, d.content FROM documents d"
        )?;
        let docs: Vec<(i64, String, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut results = Vec::new();
        for (doc_id, path, title, content) in docs {
            let mut emb_stmt = self.conn.prepare(
                "SELECT value FROM embeddings WHERE doc_id = ?1 ORDER BY dim"
            )?;
            let doc_vec: Vec<f32> = emb_stmt
                .query_map([doc_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            if doc_vec.len() < 100 { continue; }
            let sim = cosine_similarity(&query_vec, &doc_vec);
            if sim > 0.3 {
                results.push(SemanticResult { path, title, content, similarity: sim });
            }
        }
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(limit);
        Ok(results)
    }

    pub async fn add_chunks_batch(
        &mut self,
        chunks: Vec<(String, String, String, usize, usize)>,
        model: &str,
    ) -> Result<usize> {
        if chunks.is_empty() { return Ok(0); }
        let mut embeddings = Vec::new();
        for (_, _, content, _, _) in &chunks {
            embeddings.push(self.embed.embed(content, model).await?);
        }
        self.conn.execute("PRAGMA foreign_keys = OFF", [])?;
        let tx = self.conn.transaction()?;
        for ((path, title, content, start, end), embedding) in chunks.iter().zip(embeddings.iter()) {
            tx.execute(
                "INSERT OR REPLACE INTO documents (path, title, chunk_start, chunk_end, content) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![path, title, start, end, content],
            )?;
            let doc_id = tx.last_insert_rowid();
            for (dim, &value) in embedding.iter().enumerate() {
                tx.execute(
                    "INSERT OR REPLACE INTO embeddings (doc_id, dim, value) VALUES (?1, ?2, ?3)",
                    params![doc_id, dim, value],
                )?;
            }
        }
        tx.commit()?;
        self.conn.execute("PRAGMA foreign_keys = ON", [])?;
        Ok(chunks.len())
    }

    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM embeddings", [])?;
        self.conn.execute("DELETE FROM documents", [])?;
        self.conn.execute("DELETE FROM store_meta", [])?;
        Ok(())
    }

    pub fn doc_count(&self) -> Result<usize> {
        let c: i64 = self.conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))?;
        Ok(c as usize)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let dot: f32 = a[..len].iter().zip(b[..len].iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a[..len].iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b[..len].iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

/// Chunk text into pieces for embedding
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<(String, usize, usize)> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len == 0 { return chunks; }
    let mut start = 0;
    while start < len {
        let end = (start + chunk_size).min(len);
        let chunk: String = chars[start..end].iter().collect();
        chunks.push((chunk, start, end));
        if end >= len { break; }
        start += chunk_size - overlap;
    }
    chunks
}

/// Path filter - skip non-indexable content
fn should_index(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains(".tantivy_index") || s.contains(".semantic_index") { return false; }
    if s.contains("\\archive\\") || s.contains("/archive/")
        || s.contains("\\backups\\") || s.contains("/backups/") { return false; }
    if s.contains("\\logs\\") || s.contains("/logs/")
        || s.contains("\\scripts\\") || s.contains("/scripts/") { return false; }
    true
}

fn get_db_path() -> PathBuf {
    let volumes = std::env::var("VOLUMES_PATH")
        .unwrap_or_else(|_| r"C:\My Drive\Volumes".to_string());
    PathBuf::from(&volumes).join(".semantic_index.db")
}

// ============================================================================
// Tool Handlers
// ============================================================================

pub async fn handle_semantic_search(args: &Value) -> Result<Value> {
    let query = args.get("query").and_then(|q| q.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;
    let model = args.get("model").and_then(|m| m.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter — specify the embedding model used at reindex time"))?;
    let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;

    let db_path = get_db_path();
    if !db_path.exists() {
        return Ok(json!({
            "error": "Semantic index not found. Run semantic_reindex first.",
            "hint": "Call echo:semantic_reindex with a model parameter to build the embedding index"
        }));
    }

    let index = SemanticIndex::open_or_create(&db_path)?;
    let results = match index.search(query, model, limit).await {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({
                "error": e.to_string(),
                "hint": "Run semantic_reindex with the correct model to rebuild"
            }));
        }
    };

    let formatted: Vec<Value> = results.iter().map(|r| json!({
        "title": r.title,
        "path": r.path,
        "similarity": format!("{:.3}", r.similarity),
        "snippet": if r.content.len() > 200 {
            format!("{}...", r.content.chars().take(200).collect::<String>())
        } else { r.content.clone() }
    })).collect();

    Ok(json!({
        "results": formatted,
        "count": formatted.len(),
        "query": query,
        "search_type": "semantic",
        "model": model
    }))
}

pub async fn handle_semantic_reindex(args: &Value) -> Result<Value> {
    let model = args.get("model").and_then(|m| m.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter — specify the embedding model to use (e.g. 'nomic-embed-text')"))?;

    let volumes_path = std::env::var("VOLUMES_PATH")
        .unwrap_or_else(|_| r"C:\My Drive\Volumes".to_string());
    let db_path = get_db_path();

    let mut index = SemanticIndex::open_or_create(&db_path)?;

    // Discover dimension by embedding a test string — do NOT hardcode
    let probe_client = EmbedClient::new();
    let test_embedding = probe_client.embed("dimension probe", model).await
        .map_err(|e| anyhow::anyhow!(
            "Failed to connect to Ollama with model '{}': {}. Is 'ollama serve' running?", model, e
        ))?;
    let dimension = test_embedding.len();
    if dimension == 0 {
        return Ok(json!({
            "error": format!("Model '{}' returned empty embedding. Verify this model supports embeddings.", model)
        }));
    }

    // Clear existing data and record new metadata
    index.clear()?;
    index.set_metadata(model, dimension)?;

    let mut chunks_batch = Vec::new();
    let mut file_count = 0;

    for entry in WalkDir::new(&volumes_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file() && should_index(e.path()))
    {
        let ext = entry.path().extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "md" | "txt") { continue; }

        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            let path_str = entry.path().to_string_lossy().to_string();
            let title = content.lines().next()
                .map(|l| l.trim_start_matches('#').trim())
                .filter(|t| !t.is_empty() && !t.starts_with("---"))
                .unwrap_or_else(|| entry.path().file_stem()
                    .and_then(|s| s.to_str()).unwrap_or("Untitled"))
                .to_string();

            for (chunk, start, end) in chunk_text(&content, 512, 100) {
                chunks_batch.push((path_str.clone(), title.clone(), chunk, start, end));
            }
            file_count += 1;

            if chunks_batch.len() >= 100 {
                index.add_chunks_batch(std::mem::take(&mut chunks_batch), model).await?;
            }
        }
    }

    if !chunks_batch.is_empty() {
        index.add_chunks_batch(chunks_batch, model).await?;
    }

    let total_chunks = index.doc_count()?;
    Ok(json!({
        "success": true,
        "files_indexed": file_count,
        "chunks_created": total_chunks,
        "index_path": db_path.to_string_lossy(),
        "model": model,
        "dimension": dimension
    }))
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "semantic_search",
            "description": "Search by meaning similarity via Ollama embeddings. Requires Ollama running. Model must match what was used to build the index (checked against store metadata).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Natural language query"},
                    "model": {"type": "string", "description": "Embedding model (must match the model used at reindex time, e.g. 'nomic-embed-text')"},
                    "limit": {"type": "integer", "description": "Max results. Default: 10"}
                },
                "required": ["query", "model"]
            }
        }),
        json!({
            "name": "semantic_reindex",
            "description": "Rebuild semantic search index. Chunks all Volumes files and generates embeddings via Ollama. Blows away existing index. Dimension is auto-discovered from model output on first embed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model": {"type": "string", "description": "Embedding model to use (e.g. 'nomic-embed-text', 'mxbai-embed-large'). Must be pulled in Ollama first."}
                },
                "required": ["model"]
            }
        }),
    ]
}

// === FILE NAVIGATION ===
// Updated: 2026-04-11
// Total: ~330 lines | 9 functions | 5 structs | 1 constant
//
// IMPORTS: anyhow, rusqlite, serde, serde_json, std, walkdir
//
// CONSTANTS:
//   const OLLAMA_URL: ~17
//
// STRUCTS:
//   EmbedRequest: ~19-22
//   EmbedResponse: ~25-27
//   EmbedClient: ~30-32
//   pub SemanticResult: ~63-68
//   pub SemanticIndex: ~72-75
//
// IMPL BLOCKS:
//   impl EmbedClient: ~35-60
//   impl SemanticIndex: ~77-210
//
// FUNCTIONS:
//   cosine_similarity
//   pub +chunk_text
//   should_index
//   get_db_path
//   pub +handle_semantic_search
//   pub +handle_semantic_reindex
//   pub +tool_definitions
//
// === END FILE NAVIGATION ===
