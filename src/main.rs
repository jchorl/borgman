use clap::{value_t, App, Arg};

fn main() {
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
        .get_matches();

    let keep_daily = value_t!(matches, "KEEP_DAILY", u8).unwrap();
    let keep_weekly = value_t!(matches, "KEEP_WEEKLY", u8).unwrap();
    let keep_monthly = value_t!(matches, "KEEP_MONTHLY", u8).unwrap();

    println!(
        "keep daily: {}, weekly: {}, monthly: {}",
        keep_daily, keep_weekly, keep_monthly
    );
}
