use clap::{App, Arg};
use std::ffi::OsString;

pub fn get_args() -> Option<(u64, OsString, OsString)> {
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
                .value_name("SECONDS")
                .help("Timeout for web requests")
                .short("t")
                .long("timeout")
                .default_value("5"),
        )
        .get_matches();

    // Pull out command-line arguments
    let timeout = match matches.value_of("timeout").map(str::parse::<u64>) {
        None => {
            error!("Could not determine timeout value");
            return None;
        }
        Some(Err(e)) => {
            error!("Could not parse timeout as an integer: {}", e);
            return None;
        }
        Some(Ok(t)) => t,
    };
    let path = match matches.value_of_os("path") {
        None => {
            error!("Could not determine mount path");
            return None;
        }
        Some(p) => p,
    };
    let database = match matches.value_of_os("database") {
        None => {
            error!("Could not determine database location");
            return None;
        }
        Some(d) => d,
    };

    Some((timeout, path.to_owned(), database.to_owned()))
}
