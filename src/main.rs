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
use std::ffi::OsStr;
use std::time::Duration;

fn main() {
    env_logger::init();

    let (timeout, mountpoint, database) = match cli::get_args() {
        Some(args) => args,
        None => return,
    };

    let client = XkcdClient::new(Duration::from_secs(timeout), &database);

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

    match fuse::mount(fs, &mountpoint, &options) {
        Err(e) => error!("Mounting error: {}", e),
        Ok(()) => info!("Exiting gracefully"),
    }
}
