use crate::Comic;
use std::time::Duration;

mod api;
mod database;

static SQLITE_DB: &str = "/dev/shm/test.db";

#[derive(Clone, Debug)]
pub enum RequestMode {
    Normal,
    NoNetwork,
    BustCache,
    VeryFast,
}

impl RequestMode {
    pub fn network(&self) -> bool {
        match self {
            Self::Normal => true,
            Self::NoNetwork => false,
            Self::BustCache => true,
            Self::VeryFast => false,
        }
    }

    pub fn cache(&self) -> bool {
        match self {
            Self::Normal => true,
            Self::NoNetwork => true,
            Self::BustCache => false,
            Self::VeryFast => true,
        }
    }

    pub fn render(&self) -> bool {
        match self {
            Self::Normal => true,
            Self::NoNetwork => true,
            Self::BustCache => true,
            Self::VeryFast => false,
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
        debug!("Latest comic requested");

        if mode.cache() {
            trace!("Trying the cache for the latest comic");

            match database::get_latest_comic(&self.conn) {
                Ok(Some(c)) => return Some(c),
                Ok(None) => warn!("Could not find latest comic in cache"),
                Err(e) => error!("Cache error retrieving latest comic: {}", e),
            }
        } else {
            trace!(
                "Skipping the cache for the latest comic (mode was {:?})",
                mode
            );
        }

        if mode.network() {
            trace!("Trying the network for the latest comic");

            match api::get_comic(&self.client, None) {
                Ok(c) => {
                    database::insert_comic(&self.conn, &c).ok();
                    return Some(c);
                }
                Err(e) => warn!("Could not get latest comic on the network: {}", e),
            }
        } else {
            trace!(
                "Skipping the network for the latest comic (mode was {:?})",
                mode
            );
        }

        warn!("Could not find latest comic");

        None
    }

    pub fn request_comic(
        &self,
        num: u32,
        _timeout: Option<Duration>,
        mode: RequestMode,
    ) -> Option<Comic> {
        debug!("Comic {} requested", num);

        if mode.cache() {
            trace!("Trying the cache for comic {}", num);

            match database::get_comic(&self.conn, num) {
                Ok(Some(c)) => return Some(c),
                Ok(None) => info!("Comic {} not found in cache", num),
                Err(e) => error!("Error retreiving {} from cache: {}", num, e),
            }
        } else {
            trace!("Skipping the cache for comic {} (mode was {:?})", num, mode);
        }

        if mode.network() {
            trace!("Trying the network for comic {}", num);

            match api::get_comic(&self.client, Some(num)) {
                Ok(c) => {
                    database::insert_comic(&self.conn, &c).unwrap();
                    return Some(c);
                }
                Err(e) => debug!("Comic {} not found on network: {}", num, e),
            }
        } else {
            trace!(
                "Skipping the network for comic {} (mode was {:?})",
                num,
                mode
            );
        }

        None
    }

    pub fn request_raw_image(
        &self,
        comic: &Comic,
        timeout: Option<Duration>,
        mode: RequestMode,
    ) -> Option<Vec<u8>> {
        debug!("Raw image {} requested", comic);

        if mode.cache() {
            trace!("Trying the cache for raw image {}", comic);

            if let Ok(i) = database::get_raw_image(&self.conn, comic.num) {
                return Some(i);
            } else {
                debug!("Raw image {} not found in cache", comic);
            }
        } else {
            trace!(
                "Skipping the cache for raw image {} (mode was {:?})",
                comic,
                mode
            );
        }

        if mode.network() {
            match api::get_image(&self.client, &comic) {
                Ok(i) => {
                    database::insert_raw_image(&self.conn, comic.num, &i).ok();
                    return Some(i);
                }
                Err(e) => warn!(
                    "Could not get raw image {} from URL {}: {}",
                    comic, comic.img_url, e
                ),
            }
        }

        None
    }

    pub fn request_rendered_image(
        &self,
        comic: &Comic,
        timeout: Option<Duration>,
        mode: RequestMode,
    ) -> Option<Vec<u8>> {
        debug!("Rendered image {} requested", comic);

        if mode.cache() {
            trace!("Trying the cache for rendered image {}", comic);

            if let Ok(image) = database::get_rendered_image(&self.conn, comic.num) {
                return Some(image);
            }
        } else {
            trace!("Skipping the cache for rendered image {}", comic);
        }

        if mode.render() {
            trace!(
                "Getting the rendered image for {} with mode {:?}",
                comic,
                mode
            );
            let raw_image = self.request_raw_image(comic, timeout, mode)?;

            trace!("Rendering image fresh from raw image for {}", comic);

            match crate::image::render(&comic, &mut std::io::Cursor::new(&raw_image)) {
                Ok(image) => {
                    trace!("Successfully rendered {}", comic);
                    if let Err(e) = database::insert_rendered_image(&self.conn, comic.num, &image) {
                        warn!(
                            "Failed to store rendered image for {} in the cache: {}",
                            comic, e
                        );
                    }
                    return Some(image);
                }
                Err(e) => {
                    warn!("Error rendering {}: {}", comic, e);
                }
            }
        } else {
            trace!("Skipping the render for rendered image {}", comic);
        }

        None
    }
}
