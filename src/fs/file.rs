use fuse::FileType;
use std::ffi::OsStr;

/// Like an inode, but fancier
///
/// inodes are 64 bits, but are treated as two separate 32-bit fields. The
/// first field is the comic number -- it starts at 1 and goes up. The second
/// field is the type of file within each comic. Right now there's just one:
/// the image file itself.
#[derive(Debug, PartialEq)]
pub enum File {
    Root,
    Image(u32),
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
            (num, 0) if num > 0 => Some(Self::Image(num)),
            _ => None,
        }
    }

    /// Get an inode from a file
    ///
    /// Every file has an inode. In the list below, `n` is any non-zero integer.
    ///
    /// | inode Upper Half | inode Lower Half | Meaning |
    /// |--|--|--|
    /// | 0 | 0 | Root folder |
    /// | `n` | 0 | Image file `n` |
    /// | `n` | 1 | Info folder for comic `n` |
    pub fn inode(&self) -> u64 {
        match self {
            Self::Root => 1,
            Self::Image(i) => (*i as u64) << 32,
        }
    }

    pub fn from_filename<S: AsRef<OsStr>>(parent: &File, filename: S) -> Option<Self> {
        let filename: &str = filename.as_ref().to_str()?;

        match parent {
            File::Root => {
                if !filename.starts_with("xkcd_") {
                    return None;
                } else if !filename.ends_with(".png") {
                    return None;
                } else {
                    let filename = filename.split_at("xkcd_".len()).1;
                    let filename = filename.split_at(filename.len() - ".png".len()).0;

                    filename.parse().ok().map(Self::Image)
                }
            }
            _ => None,
        }
    }

    pub fn filename(&self) -> String {
        match self {
            Self::Root => "".to_string(),
            Self::Image(num) => format!("xkcd_{:05}.png", num),
        }
    }

    pub fn parent(&self) -> File {
        match self {
            Self::Root => Self::Root,
            Self::Image(_) => Self::Root,
        }
    }

    pub fn filetype(&self) -> FileType {
        match self {
            Self::Root => FileType::Directory,
            Self::Image(_) => FileType::RegularFile,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn file_from_inode() {
        assert_eq!(File::from_inode(0), None);
        assert_eq!(File::from_inode(1), Some(File::Root));
        assert_eq!(File::from_inode(2), None);

        assert_eq!(File::from_inode(0x00000001_00000000), Some(File::Image(1)));
        assert_eq!(File::from_inode(0x00000001_00000001), None);

        assert_eq!(
            File::from_inode(0xFFFFFFFF_00000000),
            Some(File::Image(0xFFFFFFFF))
        );
        assert_eq!(File::from_inode(0xFFFFFFFF_00000001), None);
    }

    #[test]
    fn file_inode_both_ways() {
        let mut interesting_numbers: Vec<u32> = Vec::new();

        interesting_numbers.extend(0..256);
        interesting_numbers.extend(0xFFFFFFF0..=0xFFFFFFFF);

        for lower_half in interesting_numbers.iter() {
            for upper_half in interesting_numbers.iter() {
                let inode: u64 = (*upper_half as u64) << 32 | (*lower_half as u64);

                let file = File::from_inode(inode);

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
        assert_eq!(File::Image(1).filename(), "xkcd_00001.png");
        assert_eq!(File::Image(123456).filename(), "xkcd_123456.png");
    }

    #[test]
    fn file_from_name() {
        assert_eq!(
            Some(File::Image(1)),
            File::from_filename(&File::Root, "xkcd_000001.png")
        );
        assert_eq!(
            Some(File::Image(123456)),
            File::from_filename(&File::Root, "xkcd_123456.png")
        );

        assert_eq!(None, File::from_filename(&File::Root, "foobar.png"));
        assert_eq!(None, File::from_filename(&File::Root, "xkcd_asdf.png"));

        assert_eq!(None, File::from_filename(&File::Image(1), ""));
    }
}
