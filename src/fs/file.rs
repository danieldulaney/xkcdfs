use fuse::FileType;
use std::ffi::OsStr;

/// Like an inode, but fancier
///
/// inodes are 64 bits, but are treated as two separate 32-bit fields. The
/// first field is the comic number -- it starts at 1 and goes up. The second
/// field is the type of file within each comic.
#[derive(Debug, PartialEq)]
pub enum File {
    Root,
    Refresh,
    Credits,
    Image(u32),
    MetaFolder(u32),
    AltText(u32),
    Title(u32),
    Transcript(u32),
    Date(u32),
    RawImage(u32),
}

impl File {
    /// Get a file from a raw inode
    ///
    /// Every file corresponds to exactly one inode, but not every inode is a
    /// valid file. See `File::inode` for the list of valid inode-file mappings.
    pub fn from_inode(ino: u64) -> Option<Self> {
        let upper_bytes: u32 = (ino >> 32) as u32;
        let lower_bytes: u32 = ino as u32;

        match (upper_bytes, lower_bytes) {
            (0, 1) => Some(Self::Root),
            (0, 2) => Some(Self::Refresh),
            (0, 3) => Some(Self::Credits),
            (0, _) => None,
            (num, 0) => Some(Self::Image(num)),
            (num, 1) => Some(Self::MetaFolder(num)),
            (num, 2) => Some(Self::AltText(num)),
            (num, 3) => Some(Self::Title(num)),
            (num, 4) => Some(Self::Transcript(num)),
            (num, 5) => Some(Self::Date(num)),
            (num, 6) => Some(Self::RawImage(num)),
            _ => None,
        }
    }

    /// Get an inode from a file
    ///
    /// Every file has an inode. In the list below, `n` is any non-zero integer.
    ///
    /// | inode Upper Half | inode Lower Half | Meaning |
    /// |-----|---|--|
    /// |  0  | 1 | Root folder |
    /// |  0  | 2 | Refresh file |
    /// |  0  | 3 | Credits file |
    /// | `n` | 0 | Image file `n` |
    /// | `n` | 1 | Metadata folder for comic `n` |
    /// | `n` | 2 | Alt-text file for comic `n` |
    /// | `n` | 3 | Title file for comic `n` |
    /// | `n` | 4 | Transcription file for comic `n` |
    /// | `n` | 5 | Date file for comic `n` |
    /// | `n` | 6 | Raw image file for comic `n` |
    pub fn inode(&self) -> u64 {
        fn from_halves(high: u32, low: u32) -> u64 {
            ((high as u64) << 32) + low as u64
        }

        match self {
            Self::Root => 1,
            Self::Refresh => 2,
            Self::Credits => 3,
            Self::Image(i) => from_halves(*i, 0),
            Self::MetaFolder(i) => from_halves(*i, 1),
            Self::AltText(i) => from_halves(*i, 2),
            Self::Title(i) => from_halves(*i, 3),
            Self::Transcript(i) => from_halves(*i, 4),
            Self::Date(i) => from_halves(*i, 5),
            Self::RawImage(i) => from_halves(*i, 6),
        }
    }

    pub fn from_filename<S: AsRef<OsStr>>(parent: &File, filename: S) -> Option<Self> {
        let filename: &str = filename.as_ref().to_str()?;

        match parent {
            File::Refresh => None,
            File::Credits => None,
            File::Image(_) => None,
            File::AltText(_) => None,
            File::Title(_) => None,
            File::Transcript(_) => None,
            File::Date(_) => None,
            File::RawImage(_) => None,
            File::Root => {
                if filename.starts_with("comic_") && filename.ends_with(".png") {
                    let filename = filename.split_at("comic_".len()).1;
                    let filename = filename.split_at(filename.len() - ".png".len()).0;

                    filename.parse().ok().map(Self::Image)
                } else if filename.starts_with("info_") {
                    let filename = filename.split_at("info_".len()).1;

                    filename.parse().ok().map(Self::MetaFolder)
                } else if filename == "refresh" {
                    Some(Self::Refresh)
                } else if filename == "credits" {
                    Some(Self::Credits)
                } else {
                    None
                }
            }
            File::MetaFolder(num) => match filename {
                "alt" => Some(Self::AltText(*num)),
                "title" => Some(Self::Title(*num)),
                "transcript" => Some(Self::Transcript(*num)),
                "date" => Some(Self::Date(*num)),
                "raw_image" => Some(Self::RawImage(*num)),
                _ => None,
            },
        }
    }

    pub fn filename(&self) -> String {
        match self {
            Self::Root => String::new(),
            Self::Refresh => String::from("refresh"),
            Self::Credits => String::from("credits"),
            Self::Image(num) => format!("comic_{:04}.png", num),
            Self::MetaFolder(num) => format!("info_{:04}", num),
            Self::AltText(_) => String::from("alt"),
            Self::Title(_) => String::from("title"),
            Self::Transcript(_) => String::from("transcript"),
            Self::Date(_) => String::from("date"),
            Self::RawImage(_) => String::from("raw_image"),
        }
    }

    pub fn filetype(&self) -> FileType {
        match self {
            Self::Root => FileType::Directory,
            Self::Refresh => FileType::RegularFile,
            Self::Credits => FileType::RegularFile,
            Self::Image(_) => FileType::RegularFile,
            Self::MetaFolder(_) => FileType::Directory,
            Self::AltText(_) => FileType::RegularFile,
            Self::Title(_) => FileType::RegularFile,
            Self::Transcript(_) => FileType::RegularFile,
            Self::Date(_) => FileType::RegularFile,
            Self::RawImage(_) => FileType::RegularFile,
        }
    }

    pub fn child_by_index(&self, index: u64, num_comics: u64) -> Option<(u64, FileType, String)> {
        match self {
            Self::Root => match index {
                0 => Some((Self::Root.inode(), Self::Root.filetype(), ".".to_string())),
                1 => Some((Self::Root.inode(), Self::Root.filetype(), "..".to_string())),
                2 => Some((
                    Self::Refresh.inode(),
                    Self::Refresh.filetype(),
                    Self::Refresh.filename(),
                )),
                3 => Some((
                    Self::Credits.inode(),
                    Self::Credits.filetype(),
                    Self::Credits.filename(),
                )),
                index if index <= (num_comics + 3) as u64 => {
                    let file = File::Image((index - 3) as u32);

                    Some((file.inode(), file.filetype(), file.filename()))
                }
                index if index <= (2 * num_comics + 3) as u64 => {
                    let file = File::MetaFolder((index - 3 - num_comics) as u32);

                    Some((file.inode(), file.filetype(), file.filename()))
                }
                _ => None,
            },
            Self::Refresh => None,
            Self::Credits => None,
            Self::Image(_) => None,
            Self::MetaFolder(num) => {
                if *num as u64 > num_comics {
                    return None;
                }

                match index {
                    0 => Some((
                        File::MetaFolder(*num).inode(),
                        File::MetaFolder(*num).filetype(),
                        ".".to_string(),
                    )),
                    1 => Some((File::Root.inode(), File::Root.filetype(), "..".to_string())),
                    2 => File::AltText(*num).triple(),
                    3 => File::Title(*num).triple(),
                    4 => File::Transcript(*num).triple(),
                    5 => File::Date(*num).triple(),
                    6 => File::RawImage(*num).triple(),
                    _ => None,
                }
            }
            Self::AltText(_) => None,
            Self::Title(_) => None,
            Self::Transcript(_) => None,
            Self::Date(_) => None,
            Self::RawImage(_) => None,
        }
    }

    /// Used to implement child_by_index
    fn triple(&self) -> Option<(u64, FileType, String)> {
        Some((self.inode(), self.filetype(), self.filename()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn file_from_inode() {
        // Root-level
        assert_eq!(File::from_inode(0), None);
        assert_eq!(File::from_inode(1), Some(File::Root));
        assert_eq!(File::from_inode(2), Some(File::Refresh));
        assert_eq!(File::from_inode(3), Some(File::Credits));
        assert_eq!(File::from_inode(4), None);

        // Image 1
        assert_eq!(File::from_inode(0x00000001_00000000), Some(File::Image(1)));
        assert_eq!(
            File::from_inode(0x00000001_00000001),
            Some(File::MetaFolder(1))
        );
        assert_eq!(
            File::from_inode(0x00000001_00000002),
            Some(File::AltText(1))
        );
        assert_eq!(File::from_inode(0x00000001_00000003), Some(File::Title(1)));
        assert_eq!(
            File::from_inode(0x00000001_00000004),
            Some(File::Transcript(1))
        );
        assert_eq!(File::from_inode(0x00000001_00000005), Some(File::Date(1)));
        assert_eq!(
            File::from_inode(0x00000001_00000006),
            Some(File::RawImage(1))
        );
        assert_eq!(File::from_inode(0x00000001_00000007), None);

        // Image 0xFFFFFFFF
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000000),
            Some(File::Image(0xFFFFFFFF))
        );
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000001),
            Some(File::MetaFolder(0xFFFFFFFF))
        );
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000002),
            Some(File::AltText(0xFFFFFFFF))
        );
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000003),
            Some(File::Title(0xFFFFFFFF))
        );
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000004),
            Some(File::Transcript(0xFFFFFFFF))
        );
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000005),
            Some(File::Date(0xFFFFFFFF))
        );
        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000006),
            Some(File::RawImage(0xFFFFFFFF))
        );
        assert_eq!(File::from_inode(0xFFFFFFFF_00000007), None);
    }

    #[test]
    fn file_inode_both_ways() {
        let mut interesting_numbers: Vec<u32> = Vec::new();

        interesting_numbers.extend(0..0x2FF);
        interesting_numbers.extend(0xFFFFFFF0..=0xFFFFFFFF);

        for lower_half in interesting_numbers.iter() {
            for upper_half in interesting_numbers.iter() {
                let inode: u64 = (*upper_half as u64) << 32 | (*lower_half as u64);

                let file = File::from_inode(inode);

                eprintln!("{:016x} -> {:?}", inode, &file);
                if let Some(ref f) = file {
                    eprintln!("{:?} -> {:016x}", f, f.inode());
                }

                match file {
                    Some(f) => assert_eq!(f.inode(), inode),
                    None => {}
                }
            }
        }
    }

    #[test]
    fn file_has_name() {
        assert_eq!(File::Root.filename(), "");

        assert_eq!(File::Refresh.filename(), "refresh");

        assert_eq!(File::Image(1).filename(), "comic_0001.png");
        assert_eq!(File::Image(123456).filename(), "comic_123456.png");

        assert_eq!(File::MetaFolder(1).filename(), "info_0001");
        assert_eq!(File::MetaFolder(123456).filename(), "info_123456");

        assert_eq!(File::AltText(1).filename(), "alt");
        assert_eq!(File::AltText(123456).filename(), "alt");
    }

    #[test]
    fn file_from_name() {
        // Successes: Parent is root
        assert_eq!(
            Some(File::Refresh),
            File::from_filename(&File::Root, "refresh")
        );
        assert_eq!(
            Some(File::Credits),
            File::from_filename(&File::Root, "credits")
        );
        assert_eq!(
            Some(File::Image(1)),
            File::from_filename(&File::Root, "comic_1.png")
        );
        assert_eq!(
            Some(File::Image(123456)),
            File::from_filename(&File::Root, "comic_123456.png")
        );
        assert_eq!(
            Some(File::MetaFolder(1)),
            File::from_filename(&File::Root, "info_1")
        );
        assert_eq!(
            Some(File::MetaFolder(123456)),
            File::from_filename(&File::Root, "info_123456")
        );

        // Failures: Parent is root
        assert_eq!(None, File::from_filename(&File::Root, "foobar.png"));
        assert_eq!(None, File::from_filename(&File::Root, "comic_asdf.png"));
        assert_eq!(None, File::from_filename(&File::Root, "info_baz"));
        assert_eq!(None, File::from_filename(&File::Root, "alt"));
        assert_eq!(None, File::from_filename(&File::Root, "title"));
        assert_eq!(None, File::from_filename(&File::Root, "transcript"));
        assert_eq!(None, File::from_filename(&File::Root, "date"));
        assert_eq!(None, File::from_filename(&File::Root, "raw_image"));

        // Successes: Parent is metafolder
        assert_eq!(
            Some(File::AltText(1)),
            File::from_filename(&File::MetaFolder(1), "alt")
        );
        assert_eq!(
            Some(File::AltText(123456)),
            File::from_filename(&File::MetaFolder(123456), "alt")
        );
        assert_eq!(
            Some(File::Title(1)),
            File::from_filename(&File::MetaFolder(1), "title")
        );
        assert_eq!(
            Some(File::Title(123456)),
            File::from_filename(&File::MetaFolder(123456), "title")
        );
        assert_eq!(
            Some(File::Transcript(1)),
            File::from_filename(&File::MetaFolder(1), "transcript")
        );
        assert_eq!(
            Some(File::Transcript(123456)),
            File::from_filename(&File::MetaFolder(123456), "transcript")
        );
        assert_eq!(
            Some(File::Date(1)),
            File::from_filename(&File::MetaFolder(1), "date")
        );
        assert_eq!(
            Some(File::Date(123456)),
            File::from_filename(&File::MetaFolder(123456), "date")
        );
        assert_eq!(
            Some(File::RawImage(1)),
            File::from_filename(&File::MetaFolder(1), "raw_image")
        );
        assert_eq!(
            Some(File::RawImage(123456)),
            File::from_filename(&File::MetaFolder(123456), "raw_image")
        );

        // Failures: Parent is a metafolder but we request a root file
        assert_eq!(
            None,
            File::from_filename(&File::MetaFolder(1), "comic_1.png")
        );
        assert_eq!(None, File::from_filename(&File::MetaFolder(1), "info_1"));
        assert_eq!(None, File::from_filename(&File::MetaFolder(1), "foobar"));

        // Failures: Parent is a regular file
        assert_eq!(None, File::from_filename(&File::Image(1), ""));
        assert_eq!(None, File::from_filename(&File::Image(123456), ""));

        assert_eq!(None, File::from_filename(&File::AltText(1), ""));
        assert_eq!(None, File::from_filename(&File::AltText(123456), ""));

        assert_eq!(None, File::from_filename(&File::Title(1), ""));
        assert_eq!(None, File::from_filename(&File::Title(123456), ""));

        assert_eq!(None, File::from_filename(&File::Transcript(1), ""));
        assert_eq!(None, File::from_filename(&File::Transcript(123456), ""));

        assert_eq!(None, File::from_filename(&File::Date(1), ""));
        assert_eq!(None, File::from_filename(&File::Date(123456), ""));

        assert_eq!(None, File::from_filename(&File::RawImage(1), ""));
        assert_eq!(None, File::from_filename(&File::RawImage(123456), ""));
    }

    fn exp_child(f: File) -> Option<(u64, FileType, String)> {
        Some((f.inode(), f.filetype(), f.filename()))
    }

    #[test]
    fn root_child_by_index_1_comic() {
        assert_eq!(
            Some((File::Root.inode(), File::Root.filetype(), ".".to_string())),
            File::Root.child_by_index(0, 1)
        );
        assert_eq!(
            Some((File::Root.inode(), File::Root.filetype(), "..".to_string())),
            File::Root.child_by_index(1, 1)
        );
        assert_eq!(exp_child(File::Refresh), File::Root.child_by_index(2, 1));
        assert_eq!(exp_child(File::Credits), File::Root.child_by_index(3, 1));
        assert_eq!(exp_child(File::Image(1)), File::Root.child_by_index(4, 1));
        assert_eq!(
            exp_child(File::MetaFolder(1)),
            File::Root.child_by_index(5, 1)
        );
        assert_eq!(None, File::Root.child_by_index(6, 1));
    }

    #[test]
    fn root_child_by_index_10000_comics() {
        assert_eq!(
            Some((File::Root.inode(), File::Root.filetype(), ".".to_string())),
            File::Root.child_by_index(0, 10_000)
        );
        assert_eq!(
            Some((File::Root.inode(), File::Root.filetype(), "..".to_string())),
            File::Root.child_by_index(1, 10_000)
        );
        assert_eq!(
            exp_child(File::Refresh),
            File::Root.child_by_index(2, 10_000)
        );
        assert_eq!(
            exp_child(File::Credits),
            File::Root.child_by_index(3, 10_000)
        );

        for i in 4..10_004 {
            assert_eq!(
                exp_child(File::Image(i - 3)),
                File::Root.child_by_index(i as u64, 10_000)
            );
        }

        for i in 10_004..20_004 {
            assert_eq!(
                exp_child(File::MetaFolder(i - 10_003)),
                File::Root.child_by_index(i as u64, 10_000)
            );
        }

        assert_eq!(None, File::Root.child_by_index(20_004, 10_000));
    }

    #[test]
    fn metafile_child_by_index() {
        assert_eq!(
            Some((
                File::MetaFolder(1).inode(),
                File::MetaFolder(1).filetype(),
                ".".to_string(),
            )),
            File::MetaFolder(1).child_by_index(0, 1)
        );

        assert_eq!(
            Some((
                File::Root.inode(),
                File::MetaFolder(1).filetype(),
                "..".to_string(),
            )),
            File::MetaFolder(1).child_by_index(1, 1)
        );

        assert_eq!(
            Some((
                File::AltText(1).inode(),
                File::AltText(1).filetype(),
                "alt".to_string(),
            )),
            File::MetaFolder(1).child_by_index(2, 1)
        );

        assert_eq!(
            Some((
                File::Title(1).inode(),
                File::Title(1).filetype(),
                "title".to_string(),
            )),
            File::MetaFolder(1).child_by_index(3, 1)
        );

        assert_eq!(
            Some((
                File::Transcript(1).inode(),
                File::Transcript(1).filetype(),
                "transcript".to_string(),
            )),
            File::MetaFolder(1).child_by_index(4, 1)
        );

        assert_eq!(
            Some((
                File::Date(1).inode(),
                File::Date(1).filetype(),
                "date".to_string(),
            )),
            File::MetaFolder(1).child_by_index(5, 1)
        );

        assert_eq!(
            Some((
                File::RawImage(1).inode(),
                File::RawImage(1).filetype(),
                "raw_image".to_string(),
            )),
            File::MetaFolder(1).child_by_index(6, 1)
        );

        assert_eq!(None, File::MetaFolder(1).child_by_index(7, 1));

        assert_eq!(None, File::MetaFolder(2).child_by_index(0, 1));
    }
}
