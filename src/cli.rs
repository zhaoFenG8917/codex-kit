use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "codex-kit",
    version,
    about = "Cross-platform system-operation CLI toolkit for AI coding agents"
)]
pub struct Cli {
    /// Output format
    #[arg(long, value_enum, global = true, default_value_t = Format::Plain)]
    pub format: Format,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Format {
    Plain,
    Json,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List directory contents
    Ls {
        /// Directory path (default: current directory)
        path: Option<PathBuf>,
        /// Show hidden files
        #[arg(short, long)]
        all: bool,
        /// Long listing format (size, modified time)
        #[arg(short, long)]
        long: bool,
    },
    /// Show directory tree
    Tree {
        /// Root directory (default: current directory)
        path: Option<PathBuf>,
        /// Maximum depth
        #[arg(short = 'd', long, default_value_t = 3)]
        depth: usize,
        /// Glob patterns to ignore, e.g. --ignore node_modules
        #[arg(long)]
        ignore: Vec<String>,
    },
    /// Show detailed metadata of a file or directory
    Info {
        /// Target path
        path: PathBuf,
    },
    /// Probe system environment (OS, tools, versions)
    Stat,
    /// Replace text in a file (reads whole file, replaces, writes back)
    Replace {
        /// Target file
        file: PathBuf,
        /// Text (or regex with --regex) to search for
        old: String,
        /// Replacement text
        new: String,
        /// Only print a diff preview, do not modify the file
        #[arg(long)]
        dry_run: bool,
        /// Treat <old> as a regular expression
        #[arg(long)]
        regex: bool,
        /// Do not create a <file>.bak backup
        #[arg(long)]
        no_backup: bool,
    },
    /// Print the first N lines of a file
    Head {
        /// Target file
        file: PathBuf,
        /// Number of lines
        #[arg(short = 'n', long, default_value_t = 20)]
        lines: usize,
    },
    /// Print the last N lines of a file
    Tail {
        /// Target file
        file: PathBuf,
        /// Number of lines
        #[arg(short = 'n', long, default_value_t = 20)]
        lines: usize,
    },
    /// Count lines, words, chars and bytes of a file
    Wc {
        /// Target file
        file: PathBuf,
    },
    /// Diff two files
    Diff {
        /// First file
        file1: PathBuf,
        /// Second file
        file2: PathBuf,
    },
    /// Run Python code with the system interpreter (fallback escape hatch)
    Exec {
        /// Python code string, or a .py file path with -f
        code: String,
        /// Treat <code> as a .py file path
        #[arg(short = 'f', long)]
        file: bool,
    },
}
