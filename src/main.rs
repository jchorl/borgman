use clap::{value_t, App, Arg, Values};
use env_logger::Env;
use log::{debug, info};
use prometheus::{Gauge, Histogram};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;

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
            InputError(path: String, message: String) {
                description("Input error")
                display("Error with input {}: {}", path, message)
            }
        }
    }
}

use errors::*;

lazy_static! {
    static ref LAST_COMPLETED_GAUGE: Gauge = register_gauge!(
        "borgman_last_completed_epoch_seconds",
        "The time of last completion"
    )
    .unwrap();
    static ref RUNTIME_HISTOGRAM: Histogram = register_histogram!(
        "borgman_run_duration_seconds",
        "The total runtime in seconds"
    )
    .unwrap();
}

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
            Arg::with_name("repo")
                .short('r')
                .long("repo")
                .value_name("PATH")
                .help("path to borg repo")
                .required(true)
                .takes_value(true),
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
        .arg(
            Arg::with_name("rclone-dest")
                .help("name of dest for rclone sync")
                .required(true)
                .takes_value(true)
                .long("rclone-dest")
                .value_name("DEST"),
        )
        .arg(
            Arg::with_name("prometheus-push-addr")
                .help("prometheus push address")
                .takes_value(true)
                .long("prometheus-push-addr")
                .value_name("ADDR"),
        )
        .get_matches();

    let _timer = RUNTIME_HISTOGRAM.start_timer();

    let matches_clone = matches.clone();
    let metrics_addr = matches_clone.value_of("prometheus-push-addr");

    if let Err(ref e) = run(matches) {
        use error_chain::ChainedError;
        use std::io::Write; // trait which holds `display_chain`
        let stderr = &mut ::std::io::stderr();
        let errmsg = "Error writing to stderr";

        writeln!(stderr, "{}", e.display_chain()).expect(errmsg);

        if let Some(ref addr) = metrics_addr {
            LAST_COMPLETED_GAUGE.set(cur_time_epoch_seconds() as f64);
            let _ = push_metrics(addr, false);
        }
        ::std::process::exit(1);
    }

    if let Some(ref addr) = metrics_addr {
        LAST_COMPLETED_GAUGE.set(cur_time_epoch_seconds() as f64);
        let _ = push_metrics(addr, true);
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

    // IMPORTANT if any inputs dont exist or are empty
    // the drive is probably not mounted and there's an issue
    // we don't want to overwrite remote state
    validate_inputs(&inputs)?;

    // first do the borg backup
    let backup_cmd = "borg";
    let mut backup_args = vec![
        "create",
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

    let repo_path = matches.value_of("repo").unwrap();
    let backup_name = String::from(repo_path) + "::'data-{now}'";
    backup_args.push(&backup_name);
    backup_args.append(&mut inputs);
    let backup_out = run_cmd(backup_cmd, backup_args, dry_run)?;
    info!("backup complete:\n{}", backup_out);

    // then prune
    let prune_cmd = "borg";
    let keep_daily_str = keep_daily.to_string();
    let keep_weekly_str = keep_weekly.to_string();
    let keep_monthly_str = keep_monthly.to_string();
    let prune_args = vec![
        "prune",
        "--list",
        "--prefix",
        "'data-'",
        "--show-rc",
        "--keep-daily",
        &keep_daily_str,
        "--keep-weekly",
        &keep_weekly_str,
        "--keep-monthly",
        &keep_monthly_str,
        repo_path,
    ];
    let prune_out = run_cmd(prune_cmd, prune_args, dry_run)?;
    info!("prune complete:\n{}", prune_out);

    // then rclone
    let rclone_cmd = "rclone";
    let rclone_dest = matches.value_of("rclone-dest").unwrap();
    let rclone_args = vec!["sync", &repo_path, rclone_dest];
    let rclone_out = run_cmd(rclone_cmd, rclone_args, dry_run)?;
    info!("rclone complete:\n{}", rclone_out);

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

// validate_inputs checks:
// 1. if the path exists
// 2. if the path is a file, all good
// 3. if its a dir, make sure it's non-empty
fn validate_inputs(paths: &Vec<&str>) -> Result<()> {
    for &path in paths {
        let attr = fs::metadata(path).chain_err(|| {
            ErrorKind::InputError(String::from(path), String::from("can't get metadata"))
        })?;
        if attr.is_file() {
            continue;
        }

        let files = fs::read_dir(path).unwrap();
        ensure!(
            files.count() > 0,
            ErrorKind::InputError(String::from(path), String::from("dir is empty"))
        )
    }

    Ok(())
}

fn cur_time_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn push_metrics(address: &str, success: bool) -> Result<()> {
    let success_str = if success { "1" } else { "0" };
    prometheus::push_metrics(
        "borgman",
        labels! {"success".to_owned() => success_str.to_owned(),},
        address,
        prometheus::gather(),
        None,
    )
    .chain_err(|| "emitting metrics")
}
