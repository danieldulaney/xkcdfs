use crate::Comic;
use std::time::Duration;

mod api;
mod database;

static SQLITE_DB: &str = "/dev/shm/test.db";

pub enum RequestMode {
    Normal,
    NoNetwork,
    NoCache,
}

impl RequestMode {
    pub fn network(&self) -> bool {
        use RequestMode::*;

        match self {
            Normal => true,
            NoNetwork => false,
            NoCache => true,
        }
    }

    pub fn cache(&self) -> bool {
        use RequestMode::*;

        match self {
            Normal => true,
            NoNetwork => true,
            NoCache => false,
        }
    }
}

pub struct XkcdClient {
    client: reqwest::Client,
    conn: rusqlite::Connection,
}

impl XkcdClient {
    pub fn new(master_timeout: Duration) -> Self {
        let new = Self {
            client: reqwest::Client::builder()
                .timeout(master_timeout)
                .build()
                .unwrap(),
            conn: rusqlite::Connection::open(SQLITE_DB).expect("Failed to connect to SQLite DB"),
        };

        database::setup(&new.conn).expect("Failed to set up SQLite DB");

        new
    }

    pub fn get_cached_count(&self) -> usize {
        database::get_comics_count(&self.conn)
    }

    pub fn get_cached_comics(&self) -> impl Iterator<Item = Option<Comic>> {
        database::get_comics(&self.conn)
    }

    pub fn request_latest_comic(
        &self,
        _timeout: Option<Duration>,
        mode: RequestMode,
    ) -> Option<Comic> {
        if mode.cache() {
            if let Some(c) = database::get_latest_comic(&self.conn) {
                return Some(c);
            }
        }

        if mode.network() {
            if let Some(c) = api::get_comic(&self.client, None) {
                database::insert_comic(&self.conn, &c).ok();
                return Some(c);
            }
        }

        None
    }

    pub fn request_comic(
        &self,
        num: u32,
        _timeout: Option<Duration>,
        mode: RequestMode,
    ) -> Option<Comic> {
        if mode.cache() {
            if let Some(c) = database::get_comic(&self.conn, num) {
                return Some(c);
            }
        }

        if mode.network() {
            if let Some(c) = api::get_comic(&self.client, Some(num)) {
                database::insert_comic(&self.conn, &c).unwrap();
                return Some(c);
            }
        }

        None
    }

    pub fn request_image(
        &self,
        num: u32,
        timeout: Option<Duration>,
        mode: RequestMode,
    ) -> Option<Vec<u8>> {
        if mode.cache() {
            if let Ok(i) = database::get_image(&self.conn, num) {
                return Some(i);
            }
        }

        if mode.network() {
            // Potentially make a network request to get the image URL
            let comic = self.request_comic(num, timeout, mode)?;

            if let Some(i) = api::get_image(&self.client, &comic) {
                database::insert_image(&self.conn, comic.num, &i).ok();
                return Some(i);
            }
        }

        None
    }
}
