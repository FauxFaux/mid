extern crate casync_http;
extern crate clap;
extern crate deb_version;
#[macro_use]
extern crate error_chain;
extern crate reqwest;
extern crate tempfile_fast;
extern crate xdg;
extern crate zstd;

mod debo;
mod errors;
mod find_repo;
mod lists;

use std::env;
use std::path;

use clap::{Arg, App, AppSettings, SubCommand};

use errors::*;

pub struct Config {
    cache_root: path::PathBuf,
    casync_mirror: String,
}

fn run() -> Result<()> {
    let matches = App::new("mid")
        .setting(AppSettings::SubcommandRequired)
        .subcommand(
            SubCommand::with_name("debo")
                .about("make a mid-style repo for a Debian source package")
                .arg(
                    Arg::with_name("SOURCE")
                        .help("the name of the source package to fetch")
                        .multiple(false)
                        .required(true),
                ),
        )
        .subcommand(SubCommand::with_name("status").about(
            "show what we think is going on",
        ))
        .get_matches();

    let dirs = xdg::BaseDirectories::with_prefix("mid").chain_err(
        || "determining XDG base directory",
    )?;

    let config = Config {
        cache_root: dirs.create_cache_directory("1").chain_err(|| {
            format!(
                "creating xdg cache directory inside {:?}",
                dirs.get_cache_home()
            )
        })?,
        casync_mirror: "https://deb-casync.goeswhere.com/".to_string(),
    };

    match matches.subcommand() {
        ("debo", Some(matches)) => {
            let pkg = matches.value_of("SOURCE").unwrap();
            debo::debo(pkg, &config).chain_err(|| {
                format!("generating debo for '{}'", pkg)
            })?;
        }
        ("status", Some(_)) => {
            show_status(&config).chain_err(|| "showing status")?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn show_status(config: &Config) -> Result<()> {
    println!("mirror:     {}", config.casync_mirror);
    println!("cache root: {:?}", config.cache_root);

    println!();

    let start = env::current_dir().chain_err(
        || "determining current directory",
    )?;

    let repos = find_repo::find_repos(&start).chain_err(|| {
        format!(
            "walking up the directory hierarchy finding repositories, start: {:?}",
            start
        )
    })?;

    if repos.is_empty() {
        println!("Not in any kind of repo.")
    } else {
        println!("In repo(s):");
        for repo in repos {
            println!(
                " * {:?} ({})",
                repo.root,
                if repo.mid { "mid" } else { "git" }
            );
        }
    }

    Ok(())
}

quick_main!(run);
