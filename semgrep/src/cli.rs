use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "semgrep",
    version,
    about = "Simple semantic code search via remote NextPlaid API",
    after_help = "ENV:\n  SEMGREP_URL   Remote API base URL (default: http://127.0.0.1:8080)\n  SEMGREP_INDEX Optional index name override",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Semantic query (not required in interactive mode)
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,

    /// Directory or file to search (default: current directory)
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Filter by language
    #[arg(long, value_enum)]
    pub lang: Option<LangArg>,

    /// Filter by extension (e.g. .py, rs)
    #[arg(long)]
    pub ext: Option<String>,

    /// Number of results
    #[arg(short = 'n', long = "num", default_value_t = 10)]
    pub num: usize,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,

    /// Interactive query mode
    #[arg(short = 'i', long)]
    pub interactive: bool,

    /// Remote API URL (env: SEMGREP_URL)
    #[arg(long)]
    pub url: Option<String>,

    /// Index name override (env: SEMGREP_INDEX)
    #[arg(long)]
    pub index: Option<String>,

    /// Show matching files only
    #[arg(short = 'l', long = "files-only")]
    pub files_only: bool,

    /// Show content snippets
    #[arg(short = 'c', long = "content")]
    pub show_content: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Initialize/index a project without searching
    Init {
        /// Directory to index (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Filter by language
        #[arg(long, value_enum)]
        lang: Option<LangArg>,

        /// Filter by extension
        #[arg(long)]
        ext: Option<String>,

        /// Remote API URL
        #[arg(long)]
        url: Option<String>,

        /// Index name override
        #[arg(long)]
        index: Option<String>,
    },

    /// Clear/remove index for a project
    Clear {
        /// Directory to clear (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Clear all indices
        #[arg(short = 'a', long)]
        all: bool,

        /// Remote API URL
        #[arg(long)]
        url: Option<String>,

        /// Index name override
        #[arg(long)]
        index: Option<String>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum LangArg {
    Python,
    Javascript,
    Typescript,
    Rust,
    Go,
}

impl LangArg {
    pub fn as_str(&self) -> &'static str {
        match self {
            LangArg::Python => "python",
            LangArg::Javascript => "javascript",
            LangArg::Typescript => "typescript",
            LangArg::Rust => "rust",
            LangArg::Go => "go",
        }
    }
}
