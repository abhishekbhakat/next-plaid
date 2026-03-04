use crate::api::SearchHit;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct DisplayResult {
    pub score: f32,
    pub document_id: i64,
    pub file_path: String,
    pub language: Option<String>,
    pub unit_type: Option<String>,
    pub name: Option<String>,
    pub signature: Option<String>,
    pub code: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

impl DisplayResult {
    fn from_hit(hit: &SearchHit) -> Self {
        Self {
            score: hit.score,
            document_id: hit.document_id,
            file_path: get_string(&hit.metadata, "file_path").unwrap_or_else(|| "<unknown>".into()),
            language: get_string(&hit.metadata, "language"),
            unit_type: get_string(&hit.metadata, "unit_type"),
            name: get_string(&hit.metadata, "name"),
            signature: get_string(&hit.metadata, "signature"),
            code: get_string(&hit.metadata, "code"),
            start_line: get_usize(&hit.metadata, "start_line"),
            end_line: get_usize(&hit.metadata, "end_line"),
        }
    }
}

pub fn to_display_results(hits: &[SearchHit]) -> Vec<DisplayResult> {
    hits.iter().map(DisplayResult::from_hit).collect()
}

pub fn print_json(results: &[DisplayResult]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(results)?);
    Ok(())
}

pub fn print_human(results: &[DisplayResult], files_only: bool, show_content: bool) {
    if results.is_empty() {
        println!("No results found.");
        return;
    }

    if files_only {
        let mut seen = HashSet::new();
        for r in results {
            if seen.insert(r.file_path.clone()) {
                println!("{}", r.file_path);
            }
        }
        return;
    }

    for (i, r) in results.iter().enumerate() {
        let rank = format!("{:>2}.", i + 1).bold();
        let loc = match (r.start_line, r.end_line) {
            (Some(s), Some(e)) if s != e => format!("{}:{}-{}", r.file_path, s, e),
            (Some(s), _) => format!("{}:{}", r.file_path, s),
            _ => r.file_path.clone(),
        };

        let label = match (&r.unit_type, &r.name) {
            (Some(t), Some(n)) => format!("{} {}", t, n),
            (Some(t), None) => t.to_string(),
            _ => r.name.clone().unwrap_or_else(|| "unit".to_string()),
        };

        println!(
            "{} {} {} {}",
            rank,
            loc.cyan(),
            format!("[{:.4}]", r.score).yellow(),
            label.bold()
        );

        if let Some(sig) = &r.signature {
            if !sig.trim().is_empty() {
                println!("    {}", sig.trim().bright_black());
            }
        }

        if show_content {
            if let Some(code) = &r.code {
                for line in code.lines().take(40) {
                    println!("    {}", line);
                }
            }
        }

        if i + 1 < results.len() {
            println!();
        }
    }
}

fn get_string(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(|s| s.to_string())
}

fn get_usize(v: &Value, key: &str) -> Option<usize> {
    v.get(key)?.as_u64().map(|x| x as usize)
}
