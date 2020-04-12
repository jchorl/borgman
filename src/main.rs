use clap::{value_t, App, Arg, Values};
use env_logger::Env;
use log::{debug, info};
use std::process::Command;

#[macro_use]
extern crate error_chain;

mod errors {
    error_chain! {
        errors {
            CommandError(cmd: String) {
                description("Command error")
                display("Running command `{}`", cmd)
            }
            CommandFailure(cmd: String, stdout: String, stderr: String) {
                description("Command failed")
                display("Running command `{}`:\n\
                stdout:\n\
                {}\n\
                stderr:\n\
                {}", cmd, stdout, stderr)
            }
        }
    }
}

use errors::*;

fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let matches = App::new("borgman")
        .version("0.0.1")
        .author("josh chorlton")
        .about("Manages the borg (https://www.borgbackup.org/)")
        .arg(
            Arg::with_name("dry-run")
                .short('n')
                .long("dry-run")
                .help("do not actually execute commands"),
        )
        .arg(
            Arg::with_name("keep-daily")
                .short('d')
                .long("keep-daily")
                .value_name("DAILY")
                .default_value("1")
                .help("number of daily archives to keep")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("keep-weekly")
                .short('w')
                .long("keep-weekly")
                .value_name("WEEKLY")
                .default_value("1")
                .help("number of weekly archives to keep")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("keep-monthly")
                .short('m')
                .long("keep-monthly")
                .value_name("MONTHLY")
                .default_value("1")
                .help("number of monthly archives to keep")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("excludes")
                .help("exclude paths matching PATTERN")
                .takes_value(true)
                .short('e')
                .long("exclude")
                .value_name("EXCLUDES")
                .multiple(true),
        )
        .arg(
            Arg::with_name("inputs")
                .help("paths to archive")
                .required(true)
                .value_name("INPUTS")
                .multiple(true),
        )
        .get_matches();

    if let Err(ref e) = run(matches) {
        use error_chain::ChainedError;
        use std::io::Write; // trait which holds `display_chain`
        let stderr = &mut ::std::io::stderr();
        let errmsg = "Error writing to stderr";

        writeln!(stderr, "{}", e.display_chain()).expect(errmsg);
        ::std::process::exit(1);
    }
}

fn run(matches: clap::ArgMatches) -> Result<()> {
    info!("starting");

    let dry_run = matches.is_present("dry-run");
    let mut inputs: Vec<&str> = matches.values_of("inputs").unwrap().collect();
    let excludes: Vec<&str> = matches
        .values_of("excludes")
        .unwrap_or(Values::default())
        .collect();
    let keep_daily = value_t!(matches, "keep-daily", u8).chain_err(|| "parsing daily flag")?;
    let keep_weekly = value_t!(matches, "keep-weekly", u8).chain_err(|| "parsing weekly flag")?;
    let keep_monthly =
        value_t!(matches, "keep-monthly", u8).chain_err(|| "parsing monthly flag")?;

    info!(
        "options: dry_run={} inputs={:?}, excludes={:?}, keep_daily={}, keep_weekly={}, keep_monthly={}",
        dry_run, inputs, excludes, keep_daily, keep_weekly, keep_monthly
    );

    // first do the borg backup
    let backup_cmd = "borg";
    let mut backup_args = vec![
        "--verbose",
        "--filter",
        "AME",
        "--list",
        "--stats",
        "--show-rc",
        "--compression",
        "lz4",
        "--exclude-caches",
    ];
    for e in excludes {
        backup_args.push("--exclude");
        backup_args.push(e);
    }
    backup_args.push("::'{hostname}-{now}'");
    backup_args.append(&mut inputs);
    let backup_out = run_cmd(backup_cmd, backup_args, dry_run)?;
    println!("{}", backup_out);

    Ok(())
}

fn run_cmd(cmd: &str, args: Vec<&str>, dry_run: bool) -> Result<String> {
    debug!("running `{} {}`", cmd, args.join(" "));

    if dry_run {
        info!("would run `{} {}`", cmd, args.join(" "));
        return Ok(String::from("not running, in dry_run mode"));
    }

    let output = Command::new(cmd)
        .args(args.clone())
        .output()
        .chain_err(|| ErrorKind::CommandError(format!("{} {}", cmd, args.clone().join(" "))))?;

    ensure!(
        output.status.success(),
        ErrorKind::CommandFailure(
            format!("{} {}", cmd, args.clone().join(" ")),
            String::from_utf8(output.stdout).unwrap_or("cant get stdout".to_string()),
            String::from_utf8(output.stderr).unwrap_or("cant get stderr".to_string())
        )
    );

    String::from_utf8(output.stdout).chain_err(|| "parsing stdout")
}
