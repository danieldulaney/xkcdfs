use rusqlite::{types::Null, Result, ToSql, NO_PARAMS};
use std::convert::TryInto;

use crate::Comic;

pub fn setup(conn: &rusqlite::Connection) -> Result<()> {
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

            img_url STRING,
            img BLOB
        );",
        std::iter::empty::<Null>(),
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
        img_len: row.get::<&str, Option<i64>>("img_len")?.map(|v| v as usize),
    })
}

pub fn get_comics(conn: &rusqlite::Connection) -> impl Iterator<Item = Option<Comic>> {
    std::iter::empty()
}

pub fn get_comics_count(conn: &rusqlite::Connection) -> usize {
    conn.query_row("SELECT max(num) FROM comics", NO_PARAMS, |row| row.get(0))
        .unwrap_or(0i64) // Return 0 on SQL error
        .try_into()
        .unwrap_or(0) // Return 0 on over (or under?) flow
}

pub fn get_latest_comic(conn: &rusqlite::Connection) -> Option<Comic> {
    None
}

pub fn get_comic(conn: &rusqlite::Connection, num: u32) -> Option<Comic> {
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
                img_url,
                length(img) as img_len
            FROM comics
            WHERE num==?;",
        )
        .ok()?;

    let mut results = statement.query_map(&[&num], row_to_comic).ok()?;

    results.next().transpose().ok()?
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

pub fn get_image(conn: &rusqlite::Connection, num: u32) -> Result<Vec<u8>> {
    let mut statement = conn
        .prepare(
            "
        SELECT img FROM comics WHERE num=?
        ;",
        )
        .unwrap();

    let data: Result<Vec<u8>> = statement.query_row(&[num], |r| r.get("img"));

    match data {
        Ok(ref d) => eprintln!("Retrieved {} bytes from cache for comic {}", d.len(), num),
        Err(ref e) => eprintln!(
            "Could not retrieve image from cache for comic {}: {}",
            num, e
        ),
    }

    data
}

pub fn insert_image(conn: &rusqlite::Connection, num: u32, data: &[u8]) -> Result<()> {
    let mut statement = conn.prepare("UPDATE comics SET img=? WHERE num=?").unwrap();

    eprintln!("Storing {} bytes in cache for comic {}", data.len(), num);

    let result = statement.execute(&[&data as &dyn ToSql, &num as &dyn ToSql]);

    dbg!(&result);

    result.map(|_| ())
}
