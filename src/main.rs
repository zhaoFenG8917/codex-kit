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
        Commands::Ls { path, all, long, limit } => {
            commands::ls::run(path.as_deref(), *all, *long, *limit, format)
        }
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
        Commands::Ps { name, limit } => commands::ps::ps(name.as_deref(), *limit, format),
        Commands::Http {
            url,
            method,
            header,
            data,
            timeout,
        } => commands::http::run(url, method, header, data.as_deref(), *timeout, format),
        Commands::Kill { pid } => commands::ps::kill(*pid, format),
        Commands::Port { port, host, timeout } => {
            commands::port::run(*port, host.as_deref(), *timeout, format)
        }
        Commands::Read { file, range } => commands::read::run(file, range, format),
        Commands::Run { cmd, timeout } => commands::run::run(cmd, *timeout, format),
        Commands::Start {
            name,
            cwd,
            log,
            wait_port,
            wait_timeout,
            cmd,
        } => commands::start::run(
            cmd,
            name.as_deref(),
            cwd.as_deref(),
            log.as_deref(),
            *wait_port,
            *wait_timeout,
            format,
        ),
        Commands::Which { command } => commands::which::run(command, format),
        Commands::Archive { output, inputs } => commands::archive::archive(output, inputs, format),
        Commands::Extract { archive, dest } => commands::archive::extract(archive, dest.as_deref(), format),
        Commands::Write { file, encoding, append } => commands::write::run(file, encoding, *append, format),
        Commands::Hash { file, algorithm } => commands::hash::run(file, algorithm, format),
        Commands::Rm { paths, force, dry_run } => {
            commands::trash::rm(paths, *force, *dry_run, format)
        }
        Commands::Trash { empty } => commands::trash::list(*empty, format),
        Commands::Restore { ids, all, overwrite } => {
            commands::trash::restore(ids, *all, *overwrite, format)
        }
        Commands::Exec { code, file } => commands::exec::run(code, *file),
    }
}
