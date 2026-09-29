mod cli;
mod commands;
mod utils;

use clap::Parser;
use cli::{Cli, Commands, Format};

fn main() {
    let cli = Cli::parse();
    let code = match dispatch(&cli.command, cli.format) {
        Ok(()) => 0,
        Err(e) => {
            utils::output::print_error(cli.format, &e.to_string());
            1
        }
    };
    std::process::exit(code);
}

fn dispatch(cmd: &Commands, format: Format) -> utils::Result<()> {
    match cmd {
        Commands::Ls { path, all, long } => commands::ls::run(path.as_deref(), *all, *long, format),
        Commands::Tree { path, depth, ignore } => {
            commands::tree::run(path.as_deref(), *depth, ignore, format)
        }
        Commands::Info { path } => commands::info::run(path, format),
        Commands::Stat => commands::stat::run(format),
        Commands::Replace {
            file,
            old,
            new,
            dry_run,
            regex,
            no_backup,
        } => commands::replace::run(file, old, new, *dry_run, *regex, *no_backup, format),
        Commands::Head { file, lines } => commands::head_tail::head(file, *lines, format),
        Commands::Tail { file, lines } => commands::head_tail::tail(file, *lines, format),
        Commands::Wc { file } => commands::wc::run(file, format),
        Commands::Diff { file1, file2 } => commands::diff::run(file1, file2, format),
        Commands::Exec { code, file } => commands::exec::run(code, *file),
    }
}
