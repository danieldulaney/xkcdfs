use clap::{App, Arg};
use log::LevelFilter;
use std::ffi::OsString;
use std::time::Duration;

pub struct Config {
    pub timeout: Duration,
    pub mountpoint: OsString,
    pub database: OsString,
    pub log_level: LevelFilter,
    pub user_agent: String,
}

pub fn get_args() -> Option<Config> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("path")
                .help("Path where the filesystem will be mounted")
                .value_name("PATH")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("database")
                .help("Database file location")
                .short("d")
                .long("database")
                .value_name("FILE")
                .default_value(":memory:")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("timeout")
                .help("Timeout for web requests")
                .value_name("SECONDS")
                .short("t")
                .long("timeout")
                .default_value("5"),
        )
        .arg(
            Arg::with_name("quiet")
                .help("Reduce output level")
                .short("q")
                .long("quiet")
                .multiple(true),
        )
        .arg(
            Arg::with_name("verbose")
                .help("Increase output level")
                .short("v")
                .long("verbose")
                .multiple(true),
        )
        .arg(
            Arg::with_name("user-agent")
                .help("User agent string to send on API requests")
                .short("a")
                .long("user-agent")
                .default_value(concat!(
                    env!("CARGO_PKG_NAME"),
                    "/",
                    env!("CARGO_PKG_VERSION")
                )),
        )
        .get_matches();

    // Pull out command-line arguments
    let timeout = match matches.value_of("timeout").map(str::parse::<u64>) {
        None => {
            panic!("Could not determine timeout value");
        }
        Some(Err(e)) => {
            panic!("Could not parse timeout as an integer: {}", e);
        }
        Some(Ok(t)) => t,
    };
    let path = match matches.value_of_os("path") {
        None => {
            panic!("Could not determine mount path");
        }
        Some(p) => p,
    };
    let database = match matches.value_of_os("database") {
        None => {
            panic!("Could not determine database location");
        }
        Some(d) => d,
    };
    let user_agent = matches.value_of("user-agent").unwrap();

    let verbosity_level: i64 =
        3 - matches.occurrences_of("quiet") as i64 + matches.occurrences_of("verbose") as i64;

    use LevelFilter::*;
    let log_level = match verbosity_level {
        std::i64::MIN..=0 => Off,
        1 => Error,
        2 => Warn,
        3 => Info,
        4 => Debug,
        5..=std::i64::MAX => Trace,
    };

    Some(Config {
        timeout: Duration::from_secs(timeout),
        mountpoint: path.to_owned(),
        database: database.to_owned(),
        log_level,
        user_agent: user_agent.to_owned(),
    })
}
