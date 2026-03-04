use crate::parser::{detect_language, parse_file, CodeUnit};
use anyhow::{anyhow, Context, Result};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use walkdir::WalkDir;

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    http: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub score: f32,
    pub document_id: i64,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub num_documents: usize,
    pub num_embeddings: usize,
    pub num_partitions: usize,
    pub avg_doclen: f64,
    pub dimension: usize,
    pub has_metadata: bool,
    pub metadata_count: Option<usize>,
    pub max_documents: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct QueryResultResponse {
    document_ids: Vec<i64>,
    scores: Vec<f32>,
    metadata: Vec<Option<Value>>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<QueryResultResponse>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    hash: u64,
    size: u64,
    modified_unix: u64,
    units: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexState {
    version: u32,
    root: String,
    index_name: String,
    files: HashMap<String, FileEntry>,
    updated_unix: u64,
}

#[derive(Debug, Clone)]
pub struct IndexSummary {
    pub changed_files: usize,
    pub removed_files: usize,
    pub indexed_files: usize,
    pub uploaded_units: usize,
}

#[derive(Debug, Clone)]
pub enum PathFilter {
    Exact(String),
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        Self {
            base_url,
            http: Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn list_indices(&self) -> Result<Vec<String>> {
        self.request_json(self.http.get(format!("{}/indices", self.base_url)))
            .await
    }

    pub async fn delete_index(&self, index_name: &str) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}/indices/{}", self.base_url, index_name))
            .send()
            .await
            .with_context(|| format!("failed to delete index '{}'", index_name))?;

        if resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(self.error_from_response(resp).await)
        }
    }

    pub async fn create_index_if_missing(&self, name: &str) -> Result<()> {
        let body = json!({
            "name": name,
            "config": {}
        });

        let resp = self
            .http
            .post(format!("{}/indices", self.base_url))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to create index '{}'", name))?;

        match resp.status() {
            s if s.is_success() => Ok(()),
            StatusCode::CONFLICT => Ok(()),
            _ => Err(self.error_from_response(resp).await),
        }
    }

    pub async fn get_index_info_optional(&self, name: &str) -> Result<Option<IndexInfo>> {
        let resp = self
            .http
            .get(format!("{}/indices/{}", self.base_url, name))
            .send()
            .await
            .with_context(|| format!("failed to fetch index info for '{}'", name))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if resp.status().is_success() {
            Ok(Some(
                resp.json::<IndexInfo>()
                    .await
                    .context("failed to decode index info")?,
            ))
        } else {
            Err(self.error_from_response(resp).await)
        }
    }

    pub async fn delete_documents_by_condition(
        &self,
        index_name: &str,
        condition: &str,
        parameters: &[Value],
    ) -> Result<()> {
        let body = json!({
            "condition": condition,
            "parameters": parameters,
        });

        self.request_unit(
            self.http
                .request(
                    reqwest::Method::DELETE,
                    format!("{}/indices/{}/documents", self.base_url, index_name),
                )
                .json(&body),
        )
        .await
    }

    pub async fn update_with_encoding(
        &self,
        index_name: &str,
        documents: &[String],
        metadata: &[Value],
    ) -> Result<()> {
        if documents.len() != metadata.len() {
            return Err(anyhow!(
                "documents length ({}) does not match metadata length ({})",
                documents.len(),
                metadata.len()
            ));
        }

        let body = json!({
            "documents": documents,
            "metadata": metadata,
            "pool_factor": 2,
        });

        self.request_unit(
            self.http
                .post(format!("{}/indices/{}/update_with_encoding", self.base_url, index_name))
                .json(&body),
        )
        .await
    }

    pub async fn search(
        &self,
        index_name: &str,
        query: &str,
        top_k: usize,
        filter: Option<(String, Vec<Value>)>,
    ) -> Result<Vec<SearchHit>> {
        if let Some((condition, params)) = filter {
            self.search_filtered_with_encoding(index_name, query, top_k, &condition, &params)
                .await
        } else {
            self.search_with_encoding(index_name, query, top_k).await
        }
    }

    async fn search_with_encoding(
        &self,
        index_name: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SearchHit>> {
        let body = json!({
            "queries": [query],
            "params": {
                "top_k": top_k,
            }
        });

        self.search_request(
            self.http
                .post(format!("{}/indices/{}/search_with_encoding", self.base_url, index_name))
                .json(&body),
        )
        .await
    }

    async fn search_filtered_with_encoding(
        &self,
        index_name: &str,
        query: &str,
        top_k: usize,
        filter_condition: &str,
        filter_parameters: &[Value],
    ) -> Result<Vec<SearchHit>> {
        let body = json!({
            "queries": [query],
            "params": {
                "top_k": top_k,
            },
            "filter_condition": filter_condition,
            "filter_parameters": filter_parameters,
        });

        self.search_request(
            self.http
                .post(format!(
                    "{}/indices/{}/search/filtered_with_encoding",
                    self.base_url, index_name
                ))
                .json(&body),
        )
        .await
    }

    async fn search_request(&self, req: RequestBuilder) -> Result<Vec<SearchHit>> {
        let body: SearchResponse = self.request_json(req).await?;

        let mut hits = Vec::new();
        if let Some(first) = body.results.into_iter().next() {
            let len = first
                .scores
                .len()
                .min(first.document_ids.len())
                .min(first.metadata.len());

            for i in 0..len {
                hits.push(SearchHit {
                    score: first.scores[i],
                    document_id: first.document_ids[i],
                    metadata: first.metadata[i].clone().unwrap_or_else(|| json!({})),
                });
            }
        }

        Ok(hits)
    }

    async fn request_json<T: DeserializeOwned>(&self, req: RequestBuilder) -> Result<T> {
        let resp = req.send().await.context("failed to send request")?;
        if resp.status().is_success() {
            resp.json::<T>()
                .await
                .context("failed to decode JSON response")
        } else {
            Err(self.error_from_response(resp).await)
        }
    }

    async fn request_unit(&self, req: RequestBuilder) -> Result<()> {
        let resp = req.send().await.context("failed to send request")?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(self.error_from_response(resp).await)
        }
    }

    async fn error_from_response(&self, resp: Response) -> anyhow::Error {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if let Ok(err) = serde_json::from_str::<ApiErrorBody>(&text) {
            if let Some(message) = err.message {
                if let Some(code) = err.code {
                    return anyhow!("API error {} ({}): {}", status, code, message);
                }
                return anyhow!("API error {}: {}", status, message);
            }
        }

        if text.is_empty() {
            anyhow!("API request failed with status {}", status)
        } else {
            anyhow!("API request failed with status {}: {}", status, text)
        }
    }
}

pub fn resolve_base_url(cli_url: Option<String>) -> String {
    cli_url
        .or_else(|| std::env::var("SEMGREP_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
}

pub fn resolve_index_name(root: &Path, cli_index: Option<String>) -> String {
    if let Some(v) = cli_index {
        return v;
    }
    if let Ok(v) = std::env::var("SEMGREP_INDEX") {
        if !v.trim().is_empty() {
            return v;
        }
    }

    let project = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let project = sanitize_index_part(project);

    let mut hasher = DefaultHasher::new();
    root.to_string_lossy().hash(&mut hasher);
    let h = hasher.finish();

    format!("semgrep-{}-{:x}", project, h)
}

pub fn resolve_root_and_path_filter(path: &Path) -> Result<(PathBuf, Option<PathFilter>)> {
    let abs = if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", path.display()))?
    } else {
        return Err(anyhow!("path does not exist: {}", path.display()));
    };

    if abs.is_file() {
        let parent = abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let file = abs
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        Ok((parent, Some(PathFilter::Exact(file))))
    } else {
        Ok((abs, None))
    }
}

pub fn build_filter(
    path_filter: Option<&PathFilter>,
    language: Option<&str>,
    extension: Option<&str>,
) -> Option<(String, Vec<Value>)> {
    let mut clauses = Vec::new();
    let mut params = Vec::new();

    if let Some(pf) = path_filter {
        match pf {
            PathFilter::Exact(p) => {
                clauses.push("file_path = ?".to_string());
                params.push(Value::String(p.clone()));
            }
        }
    }

    if let Some(lang) = language {
        clauses.push("language = ?".to_string());
        params.push(Value::String(lang.to_string()));
    }

    if let Some(ext) = extension {
        let ext = normalize_extension(ext);
        clauses.push("file_path LIKE ?".to_string());
        params.push(Value::String(format!("%{}", ext)));
    }

    if clauses.is_empty() {
        None
    } else {
        Some((clauses.join(" AND "), params))
    }
}

pub async fn ensure_indexed(
    client: &ApiClient,
    root: &Path,
    index_name: &str,
    language: Option<&str>,
    extension: Option<&str>,
) -> Result<IndexSummary> {
    client.create_index_if_missing(index_name).await?;

    let mut state = load_state(root)?.unwrap_or_else(|| IndexState {
        version: 1,
        root: root.to_string_lossy().to_string(),
        index_name: index_name.to_string(),
        files: HashMap::new(),
        updated_unix: now_unix(),
    });
    state.index_name = index_name.to_string();

    let mut changed_files = 0usize;
    let mut uploaded_units = 0usize;
    let mut next_files = HashMap::new();

    for file in list_candidate_files(root, language, extension)? {
        let rel = relative_path(root, &file)?;
        let meta = fs::metadata(&file)
            .with_context(|| format!("failed to read metadata {}", file.display()))?;
        let hash = file_hash(&file)?;
        let modified_unix = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or_default();

        let prev = state.files.get(&rel);
        let changed = prev.map(|p| p.hash != hash).unwrap_or(true);

        if changed {
            changed_files += 1;
            let units = parse_file(&file, &rel)
                .with_context(|| format!("failed parsing file {}", file.display()))?;

            // Remove stale docs for this file first.
            let _ = client
                .delete_documents_by_condition(index_name, "file_path = ?", &[Value::String(rel.clone())])
                .await;

            if !units.is_empty() {
                upload_units(client, index_name, &units).await?;
                uploaded_units += units.len();
            }

            next_files.insert(
                rel,
                FileEntry {
                    hash,
                    size: meta.len(),
                    modified_unix,
                    units: units.len(),
                },
            );
        } else if let Some(prev_entry) = prev {
            next_files.insert(rel, prev_entry.clone());
        }
    }

    let prev_set: HashSet<String> = state.files.keys().cloned().collect();
    let next_set: HashSet<String> = next_files.keys().cloned().collect();
    let removed: Vec<String> = prev_set.difference(&next_set).cloned().collect();

    for rel in &removed {
        let _ = client
            .delete_documents_by_condition(index_name, "file_path = ?", &[Value::String(rel.clone())])
            .await;
    }

    state.files = next_files;
    state.updated_unix = now_unix();
    save_state(root, &state)?;

    wait_for_index_sync(client, index_name, state_total_units(&state)).await;

    Ok(IndexSummary {
        changed_files,
        removed_files: removed.len(),
        indexed_files: state.files.len(),
        uploaded_units,
    })
}

fn state_total_units(state: &IndexState) -> usize {
    state.files.values().map(|f| f.units).sum()
}

async fn wait_for_index_sync(client: &ApiClient, index_name: &str, expected_docs: usize) {
    if expected_docs == 0 {
        return;
    }

    for _ in 0..40 {
        if let Ok(Some(info)) = client.get_index_info_optional(index_name).await {
            if info.num_documents >= expected_docs {
                return;
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn upload_units(client: &ApiClient, index_name: &str, units: &[CodeUnit]) -> Result<()> {
    let docs: Vec<String> = units.iter().map(|u| u.structured_text.clone()).collect();
    let metadata: Vec<Value> = units
        .iter()
        .map(|u| {
            json!({
                "id": u.id,
                "file_path": u.file_path,
                "language": u.language,
                "unit_type": u.unit_type,
                "name": u.name,
                "signature": u.signature,
                "code": u.code,
                "start_line": u.start_line,
                "end_line": u.end_line,
            })
        })
        .collect();

    client.update_with_encoding(index_name, &docs, &metadata).await
}

fn list_candidate_files(
    root: &Path,
    language: Option<&str>,
    extension: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let normalized_ext = extension.map(normalize_extension);

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored_dir(e.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path().to_path_buf();
        let Some(lang) = detect_language(&path) else {
            continue;
        };

        if let Some(wanted_lang) = language {
            if lang.as_str() != wanted_lang {
                continue;
            }
        }

        if let Some(wanted_ext) = &normalized_ext {
            let file_ext = path
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy().to_ascii_lowercase()))
                .unwrap_or_default();
            if &file_ext != wanted_ext {
                continue;
            }
        }

        files.push(path);
    }

    files.sort();
    Ok(files)
}

fn is_ignored_dir(path: &Path) -> bool {
    const IGNORED: &[&str] = &[
        ".git",
        ".hg",
        ".svn",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".idea",
        ".vscode",
    ];

    path.file_name()
        .and_then(|s| s.to_str())
        .map(|name| IGNORED.contains(&name))
        .unwrap_or(false)
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(root)
        .with_context(|| format!("{} is not under {}", path.display(), root.display()))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn file_hash(path: &Path) -> Result<u64> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(hasher.finish())
}

fn normalize_extension(ext: &str) -> String {
    if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{}", ext)
    }
}

fn sanitize_index_part(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    let cleaned = cleaned.trim_matches('-');
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned.to_ascii_lowercase()
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn local_states_dir() -> Result<PathBuf> {
    if let Some(d) = dirs::data_dir() {
        return Ok(d.join("semgrep").join("states"));
    }

    let home = dirs::home_dir().context("unable to determine home dir")?;
    Ok(home.join(".local").join("share").join("semgrep").join("states"))
}

pub fn get_state_file_path(root: &Path) -> Result<PathBuf> {
    let canonical = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;

    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    let digest = format!("{:x}", hasher.finish());

    let states_dir = local_states_dir()?;

    Ok(states_dir.join(format!("{}.json", digest)))
}

fn state_path(root: &Path) -> Result<PathBuf> {
    if let Ok(p) = get_state_file_path(root) {
        return Ok(p);
    }

    // Fallback if data_dir is unavailable.
    let canonical = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    let digest = format!("{:x}", hasher.finish());

    let home = dirs::home_dir().context("unable to determine home dir")?;
    Ok(home
        .join(".local")
        .join("share")
        .join("semgrep")
        .join("states")
        .join(format!("{}.json", digest)))
}

fn load_state(root: &Path) -> Result<Option<IndexState>> {
    let p = state_path(root)?;
    if !p.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&p)
        .with_context(|| format!("failed to read state file {}", p.display()))?;
    let state = serde_json::from_str::<IndexState>(&raw)
        .with_context(|| format!("failed to parse state file {}", p.display()))?;

    Ok(Some(state))
}

fn save_state(root: &Path, state: &IndexState) -> Result<()> {
    let p = state_path(root)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(state).context("failed to serialize state")?;
    fs::write(&p, raw).with_context(|| format!("failed to write state file {}", p.display()))
}
