use crate::Comic;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::convert::TryInto;

#[derive(Deserialize, Debug)]
struct ApiComic {
    num: u32,

    day: String,
    month: String,
    year: String,

    link: String,
    news: String,
    alt: String,

    title: String,
    safe_title: String,

    transcript: Option<String>,
    img: String,
}

impl TryInto<Comic> for ApiComic {
    type Error = std::num::ParseIntError;

    fn try_into(self) -> Result<Comic, Self::Error> {
        fn none_if_empty(s: String) -> Option<String> {
            match s {
                ref s if s.len() == 0 => None,
                s => Some(s),
            }
        }

        Ok(Comic {
            num: self.num,

            day: self.day.parse()?,
            month: self.month.parse()?,
            year: self.year.parse()?,

            link: none_if_empty(self.link),
            news: none_if_empty(self.news),
            alt: self.alt,

            title: self.title,
            safe_title: self.safe_title,
            transcript: self.transcript,

            img_url: self.img,
            img_len: None,
        })
    }
}

pub fn get_comic(
    client: &reqwest::Client,
    user_agent: &str,
    num: Option<u32>,
) -> Result<Comic, String> {
    let url = match num {
        Some(i) => format!("https://xkcd.com/{}/info.0.json", i),
        None => "https://xkcd.com/info.0.json".to_string(),
    };

    client
        .get(&url)
        .header(USER_AGENT, user_agent)
        .send()
        .map_err(|e| e.to_string())?
        .json::<ApiComic>()
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|e: std::num::ParseIntError| e.to_string())
}

pub fn get_image(
    client: &reqwest::Client,
    user_agent: &str,
    comic: &Comic,
) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = vec![];

    client
        .get(&comic.img_url)
        .header(USER_AGENT, user_agent)
        .send()
        .map_err(|e| e.to_string())?
        .copy_to(&mut buf)
        .map_err(|e| e.to_string())?;

    Ok(buf)
}
