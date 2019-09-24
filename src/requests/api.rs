use crate::Comic;
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

    transcript: String,
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

            img_url: self.img,
            img_len: None,
        })
    }
}

pub fn get_comic(client: &reqwest::Client, num: Option<u32>) -> Option<Comic> {
    let url = match num {
        Some(i) => format!("https://xkcd.com/{}/info.0.json", i),
        None => "https://xkcd.com/info.0.json".to_string(),
    };

    client
        .get(&url)
        .send()
        .ok()?
        .json::<ApiComic>()
        .ok()?
        .try_into()
        .ok()
}

pub fn get_image(client: &reqwest::Client, comic: &Comic) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = vec![];

    client
        .get(&comic.img_url)
        .send()
        .ok()?
        .copy_to(&mut buf)
        .ok()?;

    Some(buf)
}
