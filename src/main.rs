#[macro_use]
extern crate log;

mod cli;
mod fs;
mod image;
mod requests;
mod xkcd;

pub use fs::file::File;
pub use requests::XkcdClient;
pub use xkcd::Comic;

use requests::RequestMode::*;
use simplelog::{ConfigBuilder, SimpleLogger};
use std::ffi::OsStr;

fn main() {
    let conf = cli::get_args().unwrap();

    SimpleLogger::init(
        conf.log_level,
        ConfigBuilder::new().add_filter_allow_str("xkcdfs").build(),
    )
    .unwrap();

    let client = XkcdClient::new(conf.timeout, &conf.database, conf.user_agent);

    info!("Requesting latest comic (to get file count)");

    let latest_comic = match client.request_latest_comic(None, BustCache) {
        Some(c) => c,
        None => {
            error!("Could not fetch latest comic from https://xkcd.com");
            error!("Are you connected to the Internet?");
            return;
        }
    };

    info!("Most recent comic is {}", latest_comic);

    let fs = fs::XkcdFs::new(client);

    let options = ["-o", "fsname=xkcdfs"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();

    match fuse::mount(fs, &conf.mountpoint, &options) {
        Err(e) => error!("Mounting error: {}", e),
        Ok(()) => info!("Exiting gracefully"),
    }
}
