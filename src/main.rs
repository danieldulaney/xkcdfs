mod fs;
mod requests;
mod xkcd;

pub use requests::XkcdClient;
pub use xkcd::Comic;

use requests::RequestMode::*;
use std::env;
use std::ffi::OsStr;

fn main() {
    let client = XkcdClient::new(std::time::Duration::from_secs(1));

    dbg!(client.request_latest_comic(None, Normal));

    let fs = fs::XkcdFs::new(client);

    let mountpoint = env::args_os().nth(1).unwrap();

    let options = ["-o", "ro", "-o", "fsname=xkcd"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();

    fuse::mount(fs, &mountpoint, &options).unwrap();
}
