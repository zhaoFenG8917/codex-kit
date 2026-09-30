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
        /// Only show the first N entries
        #[arg(long)]
        limit: Option<usize>,
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
    /// List processes, optionally filtered by name
    Ps {
        /// Process name filter (case-insensitive substring)
        name: Option<String>,
        /// Only show the top N processes (sorted by memory)
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Make an HTTP request (cross-platform curl/Invoke-RestMethod alternative)
    Http {
        /// Request URL
        url: String,
        /// HTTP method
        #[arg(short = 'X', long, default_value = "GET")]
        method: String,
        /// Header, repeatable: -H "Content-Type: application/json"
        #[arg(short = 'H', long)]
        header: Vec<String>,
        /// Request body
        #[arg(short = 'd', long)]
        data: Option<String>,
        /// Timeout in seconds
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    /// Kill a process by PID
    Kill {
        /// Process ID
        pid: u32,
    },
    /// Check if a TCP port is listening (and which process owns it)
    Port {
        /// Port number
        port: u16,
    },
    /// Read a line range of a file, e.g. --range 100:200
    Read {
        /// Target file
        file: PathBuf,
        /// Line range, 1-based: "100:200", "100:", ":200", or single "150"
        #[arg(long)]
        range: String,
    },
    /// Run an external command with a timeout and capture the result
    Run {
        /// Timeout in seconds
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        /// The command and its arguments (put options like --timeout BEFORE the command)
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },
    /// Locate a command on PATH (and probe its version)
    Which {
        /// Command name
        command: String,
    },
    /// Create an archive (.zip or .tar.gz) from files/directories
    Archive {
        /// Output archive path (.zip or .tar.gz/.tgz)
        output: PathBuf,
        /// Files/directories to pack
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Extract an archive (.zip or .tar.gz)
    Extract {
        /// Archive file (.zip or .tar.gz/.tgz)
        archive: PathBuf,
        /// Destination directory (default: current directory)
        #[arg(short, long)]
        dest: Option<PathBuf>,
    },
    /// Write stdin to a file with a specific encoding
    Write {
        /// Target file
        file: PathBuf,
        /// Encoding: utf8 (default), gbk, gb18030
        #[arg(long, default_value = "utf8")]
        encoding: String,
        /// Append instead of overwrite
        #[arg(long)]
        append: bool,
    },
    /// Compute a file checksum
    Hash {
        /// Target file
        file: PathBuf,
        /// Algorithm: sha256 (default) or md5
        #[arg(short = 'a', long, default_value = "sha256")]
        algorithm: String,
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
