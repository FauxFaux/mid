extern crate casync_http;
extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate xdg;

mod errors;
mod find_repo;

use std::env;
use std::fs;
use std::io;
use std::io::Read;
use std::path;

use clap::{Arg, App, AppSettings, SubCommand};

use errors::*;

struct Config {
    cache_root: path::PathBuf,
    casync_mirror: String,
}

fn run() -> Result<()> {
    let matches = App::new("mid")
        .setting(AppSettings::SubcommandRequired)
        .subcommand(SubCommand::with_name("status").about(
            "show what we think is going on",
        ))
        .get_matches();

    let dirs = xdg::BaseDirectories::with_prefix("mid")?;
    let config = Config {
        cache_root: dirs.create_cache_directory("1")?,
        casync_mirror: "https://deb-casync.goeswhere.com/".to_string(),
    };

    match matches.subcommand() {
        ("status", Some(matches)) => {
            show_status(&config)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn show_status(config: &Config) -> Result<()> {
    println!("mirror:     {}", config.casync_mirror);
    println!("cache root: {:?}", config.cache_root);

    println!();

    let repos = find_repo::find_repos(env::current_dir()?)?;
    if repos.is_empty() {
        println!("Not in any kind of repo.")
    } else {
        println!("In repo(s):");
        for repo in repos {
            println!(" * {:?} ({})", repo.root, if repo.mid { "mid" } else { "git" });
        }
    }

    Ok(())
}

quick_main!(run);
