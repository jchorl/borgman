use clap::{value_t, App, Arg, Values};
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
    env_logger::init();

    let matches = App::new("borgman")
        .version("0.0.1")
        .author("josh chorlton")
        .about("Manages the borg (https://www.borgbackup.org/)")
        .arg(
            Arg::with_name("KEEP_DAILY")
                .short('d')
                .long("keep-daily")
                .value_name("DAILY")
                .default_value("1")
                .help("number of daily archives to keep")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("KEEP_WEEKLY")
                .short('w')
                .long("keep-weekly")
                .value_name("WEEKLY")
                .default_value("1")
                .help("number of weekly archives to keep")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("KEEP_MONTHLY")
                .short('m')
                .long("keep-monthly")
                .value_name("MONTHLY")
                .default_value("1")
                .help("number of monthly archives to keep")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("EXCLUDES")
                .help("exclude paths matching PATTERN")
                .takes_value(true)
                .short('e')
                .long("exclude")
                .multiple(true),
        )
        .arg(
            Arg::with_name("INPUTS")
                .help("paths to archive")
                .required(true)
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

    let inputs = matches.values_of("INPUTS").unwrap().collect::<String>();
    let excludes = matches
        .values_of("EXCLUDES")
        .unwrap_or(Values::default())
        .collect::<String>();
    let keep_daily = value_t!(matches, "KEEP_DAILY", u8).chain_err(|| "parsing daily flag")?;
    let keep_weekly = value_t!(matches, "KEEP_WEEKLY", u8).chain_err(|| "parsing weekly flag")?;
    let keep_monthly =
        value_t!(matches, "KEEP_MONTHLY", u8).chain_err(|| "parsing monthly flag")?;

    info!(
        "options: inputs={:?}, excludes={:?}, keep_daily={}, keep_weekly={}, keep_monthly={}",
        inputs, excludes, keep_daily, keep_weekly, keep_monthly
    );

    // first do the borg backup
    let backup_out = run_cmd("ls".to_string(), vec!["--fdsfsd".to_string()])?;
    println!("{}", backup_out);

    Ok(())
}

fn run_cmd(cmd: String, args: Vec<String>) -> Result<String> {
    debug!("running `{} {}`", cmd, args.join(" "));

    let output = Command::new(&cmd)
        .args(&args)
        .output()
        .chain_err(|| ErrorKind::CommandError(format!("{} {}", cmd, args.join(" "))))?;

    ensure!(
        output.status.success(),
        ErrorKind::CommandFailure(
            format!("{} {}", cmd, args.join(" ")),
            String::from_utf8(output.stdout).unwrap_or("cant get stdout".to_string()),
            String::from_utf8(output.stderr).unwrap_or("cant get stderr".to_string())
        )
    );

    String::from_utf8(output.stdout).chain_err(|| "parsing stdout")
}
