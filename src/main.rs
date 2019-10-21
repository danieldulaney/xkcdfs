#[macro_use]
extern crate log;

mod fs;
mod image;
mod requests;
mod xkcd;

pub use requests::XkcdClient;
pub use xkcd::Comic;

use requests::RequestMode::*;
use std::env;
use std::ffi::OsStr;

fn main() {
    env_logger::init();

    let client = XkcdClient::new(std::time::Duration::from_secs(1));

    info!("Requesting latest comic (to get file count)");

    let latest_comic = match client.request_latest_comic(None, BustCache) {
        Some(c) => c,
        None => {
            error!("Could not fetch latest comic from https://xkcd.com.");
            error!("Are you connected to the Internet?");
            return;
        }
    };

    info!("Most recent comic is {}", latest_comic);

    let fs = fs::XkcdFs::new(client);

    let mountpoint = match env::args_os().nth(1) {
        None => {
            error!("No mountpoint (use a command line argument)");
            return;
        }
        Some(s) => s,
    };

    let options = ["-o", "fsname=xkcd"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();

    match fuse::mount(fs, &mountpoint, &options) {
        Err(e) => error!("Mounting error: {}", e),
        Ok(()) => info!("Exiting gracefully"),
    }
}
