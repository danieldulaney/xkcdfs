use rusqlite::{Result, ToSql, NO_PARAMS};
use std::convert::TryInto;

use crate::Comic;

pub fn setup(conn: &rusqlite::Connection) -> Result<()> {
    info!("Setting up database");

    conn.execute(
        r"
        CREATE TABLE IF NOT EXISTS comics (
            num INTEGER PRIMARY KEY,

            day INTEGER,
            month INTEGER,
            year INTEGER,

            link STRING,
            news STRING,
            alt STRING,

            title STRING,
            safe_title STRING,

            img_url STRING
        );",
        NO_PARAMS,
    )?;

    conn.execute(
        r"
        CREATE TABLE IF NOT EXISTS raw_images (
            num INTEGER PRIMARY KEY,
            raw_image BLOB
        );",
        NO_PARAMS,
    )?;

    conn.execute(
        r"
        CREATE TABLE IF NOT EXISTS rendered_images (
            num INTEGER PRIMARY KEY,
            rendered_image BLOB
        );",
        NO_PARAMS,
    )?;

    Ok(())
}

fn row_to_comic(row: &rusqlite::Row) -> rusqlite::Result<Comic> {
    Ok(Comic {
        num: row.get("num")?,

        day: row.get("day")?,
        month: row.get("month")?,
        year: row.get("year")?,

        link: row.get("link")?,
        news: row.get("news")?,
        alt: row.get("alt")?,

        title: row.get("title")?,
        safe_title: row.get("safe_title")?,

        img_url: row.get("img_url")?,
        img_len: None,
    })
}

pub fn get_comics(conn: &rusqlite::Connection) -> impl Iterator<Item = Option<Comic>> {
    unimplemented!();
    std::iter::empty()
}

pub fn get_comics_count(conn: &rusqlite::Connection) -> usize {
    conn.query_row("SELECT max(num) FROM comics", NO_PARAMS, |row| row.get(0))
        .unwrap_or(0i64) // Return 0 on SQL error
        .try_into()
        .unwrap_or(0) // Return 0 on over (or under?) flow
}

pub fn get_latest_comic(conn: &rusqlite::Connection) -> Option<Comic> {
    unimplemented!()
}

pub fn get_comic(conn: &rusqlite::Connection, num: u32) -> Option<Comic> {
    trace!("Fetching comic {} from database", num);

    let mut statement = conn
        .prepare(
            "
            SELECT 
                num,
                day,
                month,
                year,
                link,
                news,
                alt,
                title,
                safe_title,
                img_url
            FROM comics
            WHERE num==?;",
        )
        .ok()?;

    let mut results = match statement.query_map(&[&num], row_to_comic) {
        Err(e) => {
            warn!("Database error while retrieving comic {}: {}", num, e);
            return None;
        }
        Ok(r) => r,
    };

    match results.next().transpose().ok()? {
        Some(s) => Some(s),
        None => None,
    }
}

pub fn insert_comic(conn: &rusqlite::Connection, comic: &Comic) -> Result<()> {
    let mut statement = conn
        .prepare(
            "
            INSERT OR REPLACE INTO comics (
                num,
                day,
                month,
                year,
                link,
                news,
                alt,
                title,
                safe_title,
                img_url
            ) VALUES (
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?
            );",
        )
        .unwrap();

    statement.execute(&[
        &comic.num as &dyn ToSql,
        &comic.day as &dyn ToSql,
        &comic.month as &dyn ToSql,
        &comic.year as &dyn ToSql,
        &comic.link as &dyn ToSql,
        &comic.news as &dyn ToSql,
        &comic.alt as &dyn ToSql,
        &comic.title as &dyn ToSql,
        &comic.safe_title as &dyn ToSql,
        &comic.img_url as &dyn ToSql,
    ])?;

    Ok(())
}

pub fn get_raw_image(conn: &rusqlite::Connection, num: u32) -> Result<Vec<u8>> {
    let mut statement = conn
        .prepare(
            "
            SELECT raw_image FROM raw_images WHERE num=?
            ;",
        )
        .unwrap();

    debug!("Retrieving comic {} raw image", num);

    let data: Result<Vec<u8>> = statement.query_row(&[num], |r| r.get("raw_image"));

    match data {
        Ok(ref d) => debug!(
            "Retrieved {} bytes from cache for comic {} raw image",
            d.len(),
            num
        ),
        Err(ref e) => debug!(
            "Could not retrieve raw image from cache for comic {}: {}",
            num, e
        ),
    }

    data
}

pub fn insert_raw_image(conn: &rusqlite::Connection, num: u32, data: &[u8]) -> Result<()> {
    let mut statement = conn
        .prepare("INSERT OR REPLACE INTO raw_images (num, raw_image) VALUES (?, ?)")
        .unwrap();

    debug!(
        "Storing {} bytes in cache for comic {} raw image",
        data.len(),
        num
    );

    let result = statement.execute(&[&num as &dyn ToSql, &data as &dyn ToSql]);

    result.map(|_| ())
}

pub fn get_rendered_image(conn: &rusqlite::Connection, num: u32) -> Result<Vec<u8>> {
    debug!("Retrieving comic {} rendered image", num);

    let mut statement = conn
        .prepare(
            "
            SELECT rendered_image FROM rendered_images WHERE num=?
            ;",
        )
        .unwrap();

    let data: Result<Vec<u8>> = statement.query_row(&[num], |r| r.get("rendered_image"));

    match data {
        Ok(ref d) => debug!(
            "Retrieved {} bytes from cache for comic {} rendered image",
            d.len(),
            num
        ),
        Err(ref e) => debug!(
            "Could not retrieve rendered image from cache for comic {}: {}",
            num, e
        ),
    }

    data
}

pub fn insert_rendered_image(conn: &rusqlite::Connection, num: u32, data: &[u8]) -> Result<()> {
    let mut statement = conn
        .prepare("INSERT OR REPLACE INTO rendered_images (num, rendered_image) VALUES (?, ?)")
        .unwrap();

    debug!(
        "Storing {} bytes in cache for comic {} rendered image",
        data.len(),
        num
    );

    let result = statement.execute(&[&num as &dyn ToSql, &data as &dyn ToSql]);

    result.map(|_| ())
}
