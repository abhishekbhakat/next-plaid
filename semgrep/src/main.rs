mod api;
mod cli;
mod display;
mod parser;

use anyhow::{anyhow, Result};
use clap::Parser;
use std::io::{self, Write};

use api::{
    build_filter, ensure_indexed, get_state_file_path, local_states_dir, resolve_base_url,
    resolve_index_name, resolve_root_and_path_filter, ApiClient,
};
use cli::{Cli, Commands};
use display::{print_human, print_json, to_display_results};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.clone() {
        Some(Commands::Init {
            path,
            lang,
            ext,
            url,
            index,
        }) => {
            let base_url = resolve_base_url(url);
            let (root, _) = resolve_root_and_path_filter(&path)?;
            let index_name = resolve_index_name(&root, index);
            let client = ApiClient::new(base_url);

            let language = lang.as_ref().map(|l| l.as_str());
            let summary =
                ensure_indexed(&client, &root, &index_name, language, ext.as_deref()).await?;

            println!(
                "Indexed {} files (changed: {}, removed: {}, units: {})",
                summary.indexed_files,
                summary.changed_files,
                summary.removed_files,
                summary.uploaded_units
            );
            Ok(())
        }
        Some(Commands::Clear {
            path,
            all,
            url,
            index,
        }) => {
            let base_url = resolve_base_url(url);
            let client = ApiClient::new(base_url);

            if all {
                let indices = client.list_indices().await?;
                let mut count = 0usize;

                for idx in indices {
                    if idx.starts_with("semgrep-") {
                        client.delete_index(&idx).await?;
                        count += 1;
                    }
                }

                let states_dir = local_states_dir()?;
                if states_dir.exists() {
                    for entry in std::fs::read_dir(&states_dir)? {
                        if let Ok(entry) = entry {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }

                println!("Cleared {} indices and all local state files", count);
            } else {
                let (root, _) = resolve_root_and_path_filter(&path)?;
                let index_name = resolve_index_name(&root, index);

                client.delete_index(&index_name).await?;

                let state_file = get_state_file_path(&root)?;
                if state_file.exists() {
                    std::fs::remove_file(&state_file)?;
                }

                println!("Cleared index: {}", index_name);
            }

            Ok(())
        }
        None => run_default_search(cli).await,
    }
}

async fn run_default_search(cli: Cli) -> Result<()> {
    let base_url = resolve_base_url(cli.url.clone());
    let (root, path_filter) = resolve_root_and_path_filter(&cli.path)?;
    let index_name = resolve_index_name(&root, cli.index.clone());

    let client = ApiClient::new(base_url);

    let language = cli.lang.as_ref().map(|l| l.as_str());
    let summary = ensure_indexed(&client, &root, &index_name, language, cli.ext.as_deref()).await?;
    if summary.changed_files > 0 || summary.removed_files > 0 {
        eprintln!(
            "indexed {} files (changed: {}, removed: {}, uploaded units: {})",
            summary.indexed_files,
            summary.changed_files,
            summary.removed_files,
            summary.uploaded_units
        );
    }

    let filter = build_filter(path_filter.as_ref(), language, cli.ext.as_deref());

    if cli.interactive {
        run_interactive(&client, &index_name, cli.num.max(1), filter, &cli).await
    } else {
        let query = cli
            .query
            .clone()
            .ok_or_else(|| anyhow!("query is required unless --interactive is used"))?;
        run_query(&client, &index_name, &query, cli.num.max(1), filter, &cli).await
    }
}

async fn run_interactive(
    client: &ApiClient,
    index_name: &str,
    top_k: usize,
    filter: Option<(String, Vec<serde_json::Value>)>,
    cli: &Cli,
) -> Result<()> {
    println!(
        "Interactive mode on index '{}' (server: {}). Type 'exit' to quit.",
        index_name,
        client.base_url()
    );

    let stdin = io::stdin();
    loop {
        print!("semgrep> ");
        io::stdout().flush()?;

        let mut line = String::new();
        stdin.read_line(&mut line)?;
        let query = line.trim();

        if query.is_empty() {
            continue;
        }
        if query.eq_ignore_ascii_case("exit") || query.eq_ignore_ascii_case("quit") {
            break;
        }

        run_query(client, index_name, query, top_k, filter.clone(), cli).await?;
    }

    Ok(())
}

async fn run_query(
    client: &ApiClient,
    index_name: &str,
    query: &str,
    top_k: usize,
    filter: Option<(String, Vec<serde_json::Value>)>,
    cli: &Cli,
) -> Result<()> {
    let hits = client.search(index_name, query, top_k, filter).await?;
    let results = to_display_results(&hits);

    if cli.json {
        print_json(&results)?;
    } else {
        print_human(&results, cli.files_only, cli.show_content);
    }

    Ok(())
}
