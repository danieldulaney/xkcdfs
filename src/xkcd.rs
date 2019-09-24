use time::{Timespec, Tm};

#[derive(Clone, Debug)]
pub struct Comic {
    pub num: u32,

    pub day: i32,
    pub month: i32,
    pub year: i32,

    pub link: Option<String>,
    pub news: Option<String>,
    pub alt: String,

    pub title: String,
    pub safe_title: String,

    pub img_url: String,

    pub img_len: Option<usize>,
}

impl Comic {
    pub fn time(&self) -> Timespec {
        Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 12,
            tm_mday: self.day,
            tm_mon: self.month - 1,
            tm_year: self.year - 1900,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_utcoff: 0,
            tm_nsec: 0,
        }
        .to_timespec()
    }
}
