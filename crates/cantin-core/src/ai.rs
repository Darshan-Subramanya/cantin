//! Natural-language search → [`Query`].
//!
//! The LLM never sees file contents or the file list; it only translates the
//! user's sentence into a small JSON filter that [`Query::run`] executes
//! locally. Two translators are provided:
//!
//! * [`translate`]: a local LLM server (Ollama, or any OpenAI-compatible server such
//!   as llama.cpp's `llama-server` or LM Studio) with JSON-schema constrained output.
//! * [`heuristic`]: a dependency-free rule-based parser used when no model is
//!   available, and as a sanity baseline in tests.

use crate::categories::Category;
use crate::query::{KindFilter, Query, SortBy};
use crate::units::{days_from_civil, format_date, now_unix, parse_date, parse_size};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The JSON shape the model is asked to produce. Kept deliberately small and
/// human-oriented (sizes like "1GB", dates like "2023-01-01").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiQuery {
    pub name_contains: Option<String>,
    pub extensions: Vec<String>,
    pub categories: Vec<String>,
    pub kind: Option<String>,
    pub min_size: Option<Value>,
    pub max_size: Option<Value>,
    pub modified_after: Option<String>,
    pub modified_before: Option<String>,
    pub path_contains: Option<String>,
    pub sort: Option<String>,
}

pub fn json_schema() -> Value {
    let cats: Vec<&str> = Category::ALL
        .iter()
        .map(|c| match c {
            Category::Video => "video",
            Category::Image => "image",
            Category::Audio => "audio",
            Category::Document => "document",
            Category::Archive => "archive",
            Category::Code => "code",
            Category::Installer => "installer",
            Category::DiskImage => "disk_image",
            Category::DevArtifacts => "dev_artifacts",
            Category::Cache => "cache",
            Category::Log => "log",
            Category::Other => "other",
        })
        .collect();
    let nullable_str = json!({"type": ["string", "null"]});
    json!({
        "type": "object",
        "properties": {
            "name_contains": nullable_str,
            "extensions": {"type": "array", "items": {"type": "string"}},
            "categories": {"type": "array", "items": {"type": "string", "enum": cats}},
            "kind": {"type": ["string", "null"], "enum": ["file", "dir", "any", null]},
            "min_size": nullable_str,
            "max_size": nullable_str,
            "modified_after": nullable_str,
            "modified_before": nullable_str,
            "path_contains": nullable_str,
            "sort": {"type": ["string", "null"], "enum": ["size_desc", "size_asc", "newest", "oldest", "name", null]}
        },
        "required": ["categories", "extensions"],
        "additionalProperties": false
    })
}

pub fn system_prompt(today: &str) -> String {
    format!(
        r#"You convert a request for finding files on a computer into a JSON filter.
Today is {today}. Output ONLY JSON matching the schema. Omit fields that the user did not ask for.
Fields:
- name_contains: text that must appear in the file name (not generic words like "files").
- extensions: file extensions without dot, e.g. ["pdf"].
- categories: any of video, image, audio, document, archive, code, installer, disk_image, dev_artifacts (node_modules, build folders), cache, log, other.
- kind: "file" or "dir" (folders). Default is any.
- min_size / max_size: like "500MB", "1GB".
- modified_after / modified_before: dates "YYYY-MM-DD". "not touched since 2023" or "older than 2023" means modified_before "2023-01-01". "last week" means modified_after 7 days before today.
- path_contains: a folder name the results must be inside, e.g. "Downloads".
- sort: size_desc (default), size_asc, newest, oldest, name.
Examples:
"big videos over 2 gigs" -> {{"categories":["video"],"extensions":[],"min_size":"2GB"}}
"pdfs in downloads I haven't opened since 2022" -> {{"categories":[],"extensions":["pdf"],"path_contains":"Downloads","modified_before":"2022-01-01"}}
"node_modules folders" -> {{"categories":["dev_artifacts"],"extensions":[],"kind":"dir","name_contains":"node_modules"}}
"newest screenshots" -> {{"categories":["image"],"extensions":[],"name_contains":"screenshot","sort":"newest"}}"#
    )
}

impl AiQuery {
    pub fn into_query(self) -> Query {
        let size = |v: Option<Value>| match v {
            Some(Value::Number(n)) => n.as_u64(),
            Some(Value::String(s)) => parse_size(&s),
            _ => None,
        };
        let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Query {
            name_contains: clean(self.name_contains),
            extensions: self
                .extensions
                .into_iter()
                .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
            categories: {
                let mut c: Vec<Category> =
                    self.categories.iter().filter_map(|s| Category::from_str_loose(s)).collect();
                c.dedup();
                c
            },
            kind: match self.kind.as_deref().map(str::to_ascii_lowercase).as_deref() {
                Some("file" | "files") => KindFilter::File,
                Some("dir" | "dirs" | "folder" | "folders" | "directory") => KindFilter::Dir,
                _ => KindFilter::Any,
            },
            min_size: size(self.min_size),
            max_size: size(self.max_size),
            modified_after: self.modified_after.as_deref().and_then(parse_date),
            modified_before: self.modified_before.as_deref().and_then(parse_date),
            path_contains: clean(self.path_contains),
            sort: match self.sort.as_deref() {
                Some("size_asc") => SortBy::SizeAsc,
                Some("newest") => SortBy::Newest,
                Some("oldest") => SortBy::Oldest,
                Some("name") => SortBy::Name,
                _ => SortBy::SizeDesc,
            },
            ..Default::default()
        }
    }
}

/// Extract the first JSON object from model output (tolerates code fences / prose).
pub fn parse_model_output(text: &str) -> Result<Query, AiError> {
    let start = text.find('{').ok_or(AiError::BadOutput(text.to_string()))?;
    let end = text.rfind('}').ok_or(AiError::BadOutput(text.to_string()))?;
    if end < start {
        return Err(AiError::BadOutput(text.to_string()));
    }
    let q: AiQuery = serde_json::from_str(&text[start..=end])
        .map_err(|_| AiError::BadOutput(text.to_string()))?;
    Ok(q.into_query())
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("could not reach the local model server at {0}: {1}")]
    Unreachable(String, String),
    #[error("model server error: {0}")]
    Server(String),
    #[error("the model returned something that is not a valid filter: {0}")]
    BadOutput(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Backend {
    /// Ollama (default http://localhost:11434).
    Ollama { url: String, model: String },
    /// llama.cpp `llama-server`, LM Studio, vLLM, ... (`/v1/chat/completions`).
    OpenaiCompatible { url: String, model: String },
}

impl Default for Backend {
    fn default() -> Self {
        Backend::Ollama {
            url: "http://localhost:11434".into(),
            model: "qwen2.5:3b".into(),
        }
    }
}

pub fn today() -> String {
    format_date(now_unix())
}

#[cfg(feature = "ai-http")]
pub use http::{list_models, translate};

#[cfg(feature = "ai-http")]
mod http {
    use super::*;
    use std::time::Duration;

    fn agent() -> ureq::Agent {
        // Local servers only: do not route through system HTTP proxies.
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout(Duration::from_secs(120))
            .try_proxy_from_env(false)
            .build()
    }

    fn post(url: &str, body: Value) -> Result<Value, AiError> {
        match agent().post(url).send_json(body) {
            Ok(r) => r.into_json::<Value>().map_err(|e| AiError::Server(e.to_string())),
            Err(ureq::Error::Status(code, r)) => {
                Err(AiError::Server(format!("HTTP {code}: {}", r.into_string().unwrap_or_default())))
            }
            Err(e) => Err(AiError::Unreachable(url.to_string(), e.to_string())),
        }
    }

    /// Translate a sentence to a [`Query`] using a local model server.
    pub fn translate(backend: &Backend, prompt: &str) -> Result<Query, AiError> {
        let sys = system_prompt(&today());
        let messages = json!([
            {"role": "system", "content": sys},
            {"role": "user", "content": prompt}
        ]);
        let text = match backend {
            Backend::Ollama { url, model } => {
                let v = post(
                    &format!("{}/api/chat", url.trim_end_matches('/')),
                    json!({
                        "model": model,
                        "messages": messages,
                        "stream": false,
                        "format": json_schema(),
                        "options": {"temperature": 0}
                    }),
                )?;
                v["message"]["content"].as_str().unwrap_or_default().to_string()
            }
            Backend::OpenaiCompatible { url, model } => {
                let v = post(
                    &format!("{}/v1/chat/completions", url.trim_end_matches('/')),
                    json!({
                        "model": model,
                        "messages": messages,
                        "temperature": 0,
                        "response_format": {
                            "type": "json_schema",
                            "json_schema": {"name": "file_query", "schema": json_schema()}
                        }
                    }),
                )?;
                v["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string()
            }
        };
        parse_model_output(&text)
    }

    /// Models available on the server (for the settings dropdown).
    pub fn list_models(backend: &Backend) -> Result<Vec<String>, AiError> {
        let (url, path, key) = match backend {
            Backend::Ollama { url, .. } => (url, "/api/tags", "models"),
            Backend::OpenaiCompatible { url, .. } => (url, "/v1/models", "data"),
        };
        let full = format!("{}{}", url.trim_end_matches('/'), path);
        let v: Value = match agent().get(&full).call() {
            Ok(r) => r.into_json().map_err(|e| AiError::Server(e.to_string()))?,
            Err(e) => return Err(AiError::Unreachable(full, e.to_string())),
        };
        Ok(v[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["name"].as_str().or(m["id"].as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// Rule-based fallback that understands common phrasings without any model.
pub fn heuristic(prompt: &str, today_unix: i64) -> Query {
    let mut text = prompt.to_lowercase();
    // multi-word category names → single tokens
    for (phrase, token) in [
        ("disk images", "disk_image"), ("disk image", "disk_image"),
        ("vm images", "disk_image"), ("virtual machines", "disk_image"),
        ("dev dependencies", "dev_artifacts"), ("build folders", "dev_artifacts build folders"),
        ("temp files", "temp"), ("cache files", "cache"),
    ] {
        text = text.replace(phrase, token);
    }
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == '?')
        .filter(|w| !w.is_empty())
        .collect();
    let mut q = Query::default();

    for w in &words {
        let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.');
        if let Some(c) = Category::from_str_loose(w) {
            if c != Category::Other && !q.categories.contains(&c) {
                q.categories.push(c);
            }
        }
        if let Some(ext) = w.strip_prefix('.') {
            if !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
                q.extensions.push(ext.to_string());
            }
        }
        let known_ext = ["pdf", "pdfs", "zip", "zips", "mp4", "mp4s", "mkv", "jpg", "jpgs", "png",
            "pngs", "dmg", "dmgs", "iso", "isos", "exe", "exes", "mp3", "mp3s", "psd", "csv", "log"];
        if known_ext.contains(&w) {
            let e = w.trim_end_matches('s');
            if !q.extensions.iter().any(|x| x == e) {
                q.extensions.push(e.to_string());
            }
        }
        match w {
            "folder" | "folders" | "directory" | "directories" | "dirs" => q.kind = KindFilter::Dir,
            "node_modules" => {
                q.kind = KindFilter::Dir;
                q.name_contains = Some("node_modules".into());
            }
            "screenshot" | "screenshots" => {
                q.name_contains = Some("screenshot".into());
                if !q.categories.contains(&Category::Image) {
                    q.categories.push(Category::Image);
                }
            }
            "newest" | "latest" | "recent" | "recently" => q.sort = SortBy::Newest,
            "oldest" => q.sort = SortBy::Oldest,
            "smallest" => q.sort = SortBy::SizeAsc,
            _ => {}
        }
    }
    // "log" as category and extension is redundant
    if q.categories.contains(&Category::Log) {
        q.extensions.retain(|e| e != "log");
    }

    // sizes: "over 1gb", "bigger than 500 mb", "> 2g", "under 10mb"
    let joined = words.join(" ");
    let size_after = |markers: &[&str]| -> Option<u64> {
        for m in markers {
            if let Some(pos) = joined.find(m) {
                let rest = joined[pos + m.len()..].trim_start();
                let mut tok: String = rest.split(' ').take(1).collect();
                // "500 mb" → join number and unit
                if tok.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    if let Some(unit) = rest.split(' ').nth(1) {
                        tok.push_str(unit);
                    }
                }
                let tok = tok.replace("gigs", "gb").replace("gig", "gb").replace("megs", "mb");
                if let Some(s) = parse_size(&tok) {
                    return Some(s);
                }
            }
        }
        None
    };
    q.min_size = size_after(&["over ", "bigger than ", "larger than ", "more than ", "above ", "> ", "at least "]);
    q.max_size = size_after(&["under ", "smaller than ", "less than ", "below ", "< ", "at most "]);
    if q.min_size.is_none() && words.iter().any(|w| matches!(*w, "big" | "huge" | "large" | "biggest" | "largest")) {
        q.min_size = Some(100 * 1024 * 1024);
    }

    // dates
    let year_after = |markers: &[&str]| -> Option<i64> {
        for m in markers {
            if let Some(pos) = joined.find(m) {
                let rest = &joined[pos + m.len()..];
                if let Some(tok) = rest.split(' ').next() {
                    if let Some(t) = parse_date(tok) {
                        return Some(t);
                    }
                }
            }
        }
        None
    };
    let untouched = ["haven't touched since ", "not touched since ", "haven't opened since ",
        "not opened since ", "unused since ", "not modified since ", "untouched since "];
    if let Some(t) = year_after(&untouched) {
        q.modified_before = Some(t);
    } else {
        q.modified_after = year_after(&["since ", "after ", "newer than "]);
    }
    if q.modified_before.is_none() {
        q.modified_before = year_after(&["before ", "older than "]);
    }
    let day = 86_400;
    if joined.contains("last week") || joined.contains("this week") {
        q.modified_after = Some(today_unix - 7 * day);
    } else if joined.contains("last month") || joined.contains("this month") {
        q.modified_after = Some(today_unix - 30 * day);
    } else if joined.contains("today") {
        q.modified_after = Some(today_unix.div_euclid(day) * day);
    } else if joined.contains("last year") {
        let (y, _, _) = crate::units::civil_from_days(today_unix.div_euclid(day));
        q.modified_after = Some(days_from_civil(y - 1, 1, 1) * day);
        q.modified_before = Some(days_from_civil(y, 1, 1) * day);
    }

    // "in downloads", "inside Desktop"
    for m in ["in my ", "inside my ", "in ", "inside ", "under "] {
        if let Some(pos) = joined.find(m) {
            let tok = joined[pos + m.len()..].split(' ').next().unwrap_or("");
            let known = ["downloads", "desktop", "documents", "pictures", "movies", "music", "videos", "library", "appdata", "projects"];
            if known.contains(&tok) {
                // preserve user's capitalisation from the original prompt
                let orig = prompt
                    .split_whitespace()
                    .find(|w| w.eq_ignore_ascii_case(tok))
                    .unwrap_or(tok);
                q.path_contains = Some(orig.to_string());
                // "documents"/"videos"/"pictures" here is a place, not a category
                if let Some(c) = Category::from_str_loose(tok) {
                    q.categories.retain(|x| *x != c);
                }
                break;
            }
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn d(s: &str) -> i64 {
        parse_date(s).unwrap()
    }

    #[test]
    fn model_output_parsing() {
        let q = parse_model_output(
            "```json\n{\"categories\":[\"video\"],\"extensions\":[],\"min_size\":\"1GB\",\"modified_before\":\"2023-01-01\"}\n```",
        )
        .unwrap();
        assert_eq!(q.categories, vec![Category::Video]);
        assert_eq!(q.min_size, Some(1 << 30));
        assert_eq!(q.modified_before, Some(d("2023-01-01")));

        let q = parse_model_output(r#"{"extensions":[".PDF"],"kind":"folder","min_size":2048}"#).unwrap();
        assert_eq!(q.extensions, vec!["pdf"]);
        assert_eq!(q.kind, KindFilter::Dir);
        assert_eq!(q.min_size, Some(2048));

        assert!(parse_model_output("sorry, I can't").is_err());
    }

    #[test]
    fn schema_is_valid_json_object() {
        let s = json_schema();
        assert_eq!(s["type"], "object");
        assert!(s["properties"]["categories"]["items"]["enum"].as_array().unwrap().len() == 12);
        assert!(system_prompt("2026-01-01").contains("2026-01-01"));
    }

    /// Golden set for the rule-based translator. The same prompts are used to
    /// evaluate LLM backends (see `cantin ask --eval`).
    #[test]
    fn heuristic_golden() {
        let now = d("2026-11-07");
        let cases: Vec<(&str, Query)> = vec![
            ("videos over 1GB I haven't touched since 2023", Query {
                categories: vec![Category::Video], min_size: Some(1 << 30),
                modified_before: Some(d("2023-01-01")), ..Default::default() }),
            ("pdfs in Downloads", Query {
                extensions: vec!["pdf".into()], path_contains: Some("Downloads".into()), ..Default::default() }),
            ("node_modules folders", Query {
                categories: vec![Category::DevArtifacts], kind: KindFilter::Dir,
                name_contains: Some("node_modules".into()), ..Default::default() }),
            ("photos bigger than 20 mb", Query {
                categories: vec![Category::Image], min_size: Some(20 << 20), ..Default::default() }),
            ("newest screenshots", Query {
                categories: vec![Category::Image], name_contains: Some("screenshot".into()),
                sort: SortBy::Newest, ..Default::default() }),
            ("music files under 5mb", Query {
                categories: vec![Category::Audio], max_size: Some(5 << 20), ..Default::default() }),
            ("logs from last week", Query {
                categories: vec![Category::Log], modified_after: Some(now - 7 * DAY), ..Default::default() }),
            ("disk images older than 2020", Query {
                categories: vec![Category::DiskImage], modified_before: Some(d("2020-01-01")), ..Default::default() }),
            ("huge archives", Query {
                categories: vec![Category::Archive], min_size: Some(100 << 20), ..Default::default() }),
            ("documents in my Desktop since 2025-06", Query {
                categories: vec![Category::Document], path_contains: Some("Desktop".into()),
                modified_after: Some(d("2025-06-01")), ..Default::default() }),
        ];
        for (prompt, expected) in cases {
            let got = heuristic(prompt, now);
            assert_eq!(got, expected, "prompt: {prompt}");
        }
    }
}
