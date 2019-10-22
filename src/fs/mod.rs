pub mod file;

use fuse::{
    FileAttr, Filesystem, ReplyAttr, ReplyData, ReplyEntry, ReplyOpen, ReplyWrite, Request,
};
use libc::{EINVAL, EISDIR, ENOENT, ENOTDIR, EPERM, EREMOTEIO};
use std::convert::TryInto;
use std::ffi::OsStr;
use time::Timespec;

use crate::{requests::RequestMode::*, Comic};
use file::File;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };
const EPOCH: Timespec = Timespec { sec: 0, nsec: 0 };
const GEN: u64 = 0;
const BLOCK_SIZE: u64 = 512;
const DIR_SIZE: u64 = 4096;
const DEFAULT_SIZE: u64 = 4096;
const DEFAULT_PERM: u16 = 0o444;

const CREDITS_DATA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/credits.txt"));

pub struct XkcdFs {
    client: crate::XkcdClient,
    next_fh: u64,
}

impl XkcdFs {
    pub fn new(client: crate::XkcdClient) -> Self {
        Self { client, next_fh: 1 }
    }

    const fn blocks(size: u64) -> u64 {
        (size + BLOCK_SIZE - 1) / BLOCK_SIZE
    }

    fn gen_fh(&mut self) -> u64 {
        let fh = self.next_fh;

        self.next_fh = self.next_fh.wrapping_add(1);

        fh
    }

    fn file_attr(&self, request: &Request, file: File) -> Option<FileAttr> {
        info!("Getting attributes for {:?}", file);

        let rdev = 0;
        let flags = 0;
        let nlink = 0;

        let attrs = |size: Option<usize>, time: Option<Timespec>| {
            let time = time.unwrap_or(EPOCH);
            let size = size.map(|s| s as u64).unwrap_or(DEFAULT_SIZE);

            Some(FileAttr {
                ino: file.inode(),
                size,
                blocks: Self::blocks(size),
                atime: time,
                mtime: time,
                ctime: time,
                crtime: time,
                kind: file.filetype(),
                perm: DEFAULT_PERM,
                nlink,
                uid: request.uid(),
                gid: request.gid(),
                rdev,
                flags,
            })
        };

        match file {
            File::Root => Some(FileAttr {
                ino: file.inode(),
                size: DIR_SIZE,
                blocks: Self::blocks(DIR_SIZE),
                atime: Timespec::new(0, 0),
                mtime: Timespec::new(0, 0),
                ctime: Timespec::new(0, 0),
                crtime: Timespec::new(0, 0),
                kind: file.filetype(),
                perm: DEFAULT_PERM,
                nlink,
                uid: request.uid(),
                gid: request.gid(),
                rdev,
                flags,
            }),
            File::Refresh => Some(FileAttr {
                ino: file.inode(),
                size: 0,
                blocks: 1,
                atime: Timespec::new(0, 0),
                mtime: Timespec::new(0, 0),
                ctime: Timespec::new(0, 0),
                crtime: Timespec::new(0, 0),
                kind: file.filetype(),
                perm: 0o666,
                nlink,
                uid: request.uid(),
                gid: request.gid(),
                rdev,
                flags,
            }),
            File::Credits => attrs(Some(CREDITS_DATA.len()), None),
            File::Image(num) => {
                let comic: Option<Comic> = self.client.request_comic(num, None, VeryFast);
                let image = comic
                    .as_ref()
                    .and_then(|c| self.client.request_rendered_image(&c, None, VeryFast));

                debug!(
                    "Rendered image has size {:?}",
                    image.as_ref().map(|i| i.len())
                );

                attrs(image.map(|i| i.len()), comic.map(|c| c.time()))
            }
            File::MetaFolder(num) => {
                let comic: Option<Comic> = self.client.request_comic(num, None, VeryFast);

                let time = comic.map(|c| c.time()).unwrap_or(EPOCH);

                Some(FileAttr {
                    ino: file.inode(),
                    size: DIR_SIZE,
                    blocks: Self::blocks(DIR_SIZE),
                    atime: time,
                    mtime: time,
                    ctime: time,
                    crtime: time,
                    kind: file.filetype(),
                    perm: DEFAULT_PERM,
                    nlink,
                    uid: request.uid(),
                    gid: request.gid(),
                    rdev,
                    flags,
                })
            }
            File::AltText(num) => {
                let comic = self.client.request_comic(num, None, VeryFast);

                attrs(comic.as_ref().map(|c| c.alt.len()), comic.map(|c| c.time()))
            }
            File::Title(num) => {
                let comic = self.client.request_comic(num, None, VeryFast);

                attrs(
                    comic.as_ref().map(|c| c.title.len()),
                    comic.map(|c| c.time()),
                )
            }
            File::Transcript(num) => {
                let comic = self.client.request_comic(num, None, VeryFast);

                attrs(
                    comic
                        .as_ref()
                        .and_then(|c| c.transcript.as_ref().map(|t| t.len())),
                    comic.map(|c| c.time()),
                )
            }
            File::Date(num) => {
                let comic = self.client.request_comic(num, None, VeryFast);

                attrs(
                    comic.as_ref().map(|c| c.isodate().len()),
                    comic.map(|c| c.time()),
                )
            }
            File::RawImage(num) => {
                let comic: Option<Comic> = self.client.request_comic(num, None, VeryFast);
                let raw_image = comic
                    .as_ref()
                    .and_then(|c| self.client.request_raw_image(&c, None, VeryFast));

                attrs(raw_image.map(|i| i.len()), comic.map(|c| c.time()))
            }
        }
    }
}

impl<'q> Filesystem for XkcdFs {
    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        let file = File::from_inode(ino);

        match &file {
            Some(f) => info!("getattr for {:?}", f),
            None => warn!("getattr for invalid inode {:x}", ino),
        }

        let attr = file.and_then(|f| self.file_attr(req, f));

        match attr {
            None => reply.error(ENOENT),
            Some(attr) => reply.attr(&TTL, &attr),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: fuse::ReplyDirectory,
    ) {
        let file = File::from_inode(ino);

        match &file {
            Some(f) => info!("readdir for {:?} at offset {}", f, offset),
            None => warn!("readdir for invalid inode {:x} at offset {}", ino, offset),
        }

        let file = match file {
            Some(f @ File::Root) => f,
            Some(f @ File::MetaFolder(_)) => f,
            Some(File::Refresh)
            | Some(File::Credits)
            | Some(File::Image(_))
            | Some(File::AltText(_))
            | Some(File::Title(_))
            | Some(File::Transcript(_))
            | Some(File::Date(_))
            | Some(File::RawImage(_)) => {
                reply.error(ENOTDIR);
                return;
            }
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let mut current: u64 = offset as u64;
        let comic_count: u64 = self.client.get_cached_count() as u64;

        loop {
            let child = file.child_by_index(current, comic_count);

            let done = match child {
                None => break,
                Some((ino, filetype, filename)) => {
                    reply.add(ino, (current + 1) as i64, filetype, filename)
                }
            };

            if done {
                break;
            }

            current += 1;
        }

        reply.ok();
    }

    fn lookup(&mut self, req: &Request, parent_ino: u64, name: &OsStr, reply: ReplyEntry) {
        let parent = File::from_inode(parent_ino);

        match &parent {
            Some(p) => info!("lookup for {:?} with parent {:?}", name, p),
            None => warn!(
                "lookup for {:?} with invalid parent inode {}",
                name, parent_ino
            ),
        }

        let attr = parent
            .and_then(|p| File::from_filename(&p, name))
            .and_then(|f| self.file_attr(req, f));

        match attr {
            Some(a) => reply.entry(&TTL, &a, GEN),
            None => reply.error(ENOENT),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        reply: ReplyData,
    ) {
        let file = File::from_inode(ino);

        match &file {
            Some(f) => info!("read for {:?} at {} size {}", f, offset, size),
            None => warn!(
                "read for invalid inode {:x} at {} size {}",
                ino, offset, size
            ),
        }

        // Utility function that handles some of the edge cases related to
        // converting a slice into a response
        let reply_from_slice = |bytes: Result<&[u8], i32>| {
            let bytes = match bytes {
                Ok(b) => b,
                Err(code) => {
                    reply.error(code);
                    return;
                }
            };

            let offset_usize: usize = offset.try_into().unwrap();

            let range_end = std::cmp::min(offset_usize + size as usize, bytes.len());

            if offset >= bytes.len() as i64 {
                // Start of request is beyond the end of the range
                reply.error(EINVAL);
            } else if range_end <= offset_usize {
                // Range ends before it begins
                reply.error(EINVAL);
            } else {
                reply.data(&bytes[offset_usize..range_end]);
            }
        };

        match file {
            Some(File::Image(num)) => {
                debug!("Requesting image file for comic {}", num);

                let comic = self.client.request_comic(num, None, Normal);
                let image =
                    comic.and_then(|c| self.client.request_rendered_image(&c, None, Normal));

                reply_from_slice(image.as_ref().map(Vec::as_slice).ok_or(EREMOTEIO))
            }
            Some(File::AltText(num)) => {
                debug!("Requesting comic for alt text {}", num);

                let comic = self.client.request_comic(num, None, Normal);
                let string = comic.map(|c| c.alt);
                let bytes = string.as_ref().map(String::as_bytes);

                reply_from_slice(bytes.ok_or(EREMOTEIO))
            }
            Some(File::Credits) => reply_from_slice(Ok(CREDITS_DATA.as_bytes())),
            Some(File::Refresh) => {
                debug!("Refreshing latest comic");
                reply_from_slice(Ok(&[]))
            }
            Some(File::Title(num)) => {
                let comic = self.client.request_comic(num, None, Normal);
                let string = comic.map(|c| c.title);
                let bytes = string.as_ref().map(String::as_bytes);

                reply_from_slice(bytes.ok_or(EREMOTEIO))
            }
            Some(File::Transcript(num)) => {
                let comic = self.client.request_comic(num, None, Normal);
                let string = comic.and_then(|c| c.transcript);
                let bytes = string.as_ref().map(String::as_bytes);

                reply_from_slice(bytes.ok_or(EREMOTEIO))
            }
            Some(File::Date(num)) => {
                let comic = self.client.request_comic(num, None, Normal);
                let string = comic.map(|c| c.isodate());
                let bytes = string.as_ref().map(String::as_bytes);

                reply_from_slice(bytes.ok_or(EREMOTEIO))
            }
            Some(File::RawImage(num)) => {
                let comic = self.client.request_comic(num, None, Normal);
                let raw_image = comic.and_then(|c| self.client.request_raw_image(&c, None, Normal));

                reply_from_slice(raw_image.as_ref().map(Vec::as_slice).ok_or(EREMOTEIO));
            }
            Some(f @ File::Root) | Some(f @ File::MetaFolder(_)) => {
                warn!("{:?} is a directory, returning EISDIR", f);

                reply_from_slice(Err(EISDIR))
            }
            None => {
                warn!("File does not exist, returning ENOENT");
                reply_from_slice(Err(ENOENT))
            }
        };
    }

    fn open(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        use File::*;
        const DEFAULT_FLAGS: u32 = 0;

        let file = File::from_inode(ino);

        match &file {
            Some(f) => info!("open for {:?}", f),
            None => warn!("open for invalid inode {:x}", ino),
        }

        match file {
            Some(Root) | Some(MetaFolder(_)) => reply.error(EISDIR),
            Some(Refresh) | Some(Credits) => reply.opened(self.gen_fh(), DEFAULT_FLAGS),
            Some(AltText(num)) | Some(Title(num)) | Some(Transcript(num)) | Some(Date(num)) => {
                match self.client.request_comic(num, None, Normal) {
                    Some(_) => reply.opened(self.gen_fh(), DEFAULT_FLAGS),
                    None => reply.error(EREMOTEIO),
                }
            }
            Some(Image(num)) => match self
                .client
                .request_comic(num, None, Normal)
                .and_then(|c| self.client.request_rendered_image(&c, None, Normal))
            {
                Some(_) => reply.opened(self.gen_fh(), DEFAULT_FLAGS),
                None => reply.error(EREMOTEIO),
            },
            Some(RawImage(num)) => match self
                .client
                .request_comic(num, None, Normal)
                .and_then(|c| self.client.request_raw_image(&c, None, Normal))
            {
                Some(_) => reply.opened(self.gen_fh(), DEFAULT_FLAGS),
                None => reply.error(EREMOTEIO),
            },
            None => reply.error(ENOENT),
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        _offset: i64,
        data: &[u8],
        _flags: u32,
        reply: ReplyWrite,
    ) {
        let file = File::from_inode(ino);

        match &file {
            Some(f) => info!("write for {:?} with {} bytes of data", f, data.len()),
            None => warn!(
                "write for invalid inode {:x} with {} bytes of data",
                ino,
                data.len()
            ),
        }

        match file {
            Some(File::Refresh) => {
                info!("Refreshing latest comic (via write)");

                self.client.request_latest_comic(None, BustCache);

                reply.written(data.len() as u32);
            }
            Some(_) => reply.error(EPERM),
            None => reply.error(ENOENT),
        }
    }

    fn setattr(
        &mut self,
        req: &Request,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<Timespec>,
        _mtime: Option<Timespec>,
        _fh: Option<u64>,
        _crtime: Option<Timespec>,
        _chgtime: Option<Timespec>,
        _bkuptime: Option<Timespec>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let file = File::from_inode(ino);

        match &file {
            Some(f) => info!("setattr for {:?}", f),
            None => warn!("setattr for invalid inode {:x}", ino),
        }

        match file {
            Some(File::Refresh) => {
                info!("Refreshing latest comic (via setattr)");

                self.client.request_latest_comic(None, BustCache);

                self.getattr(req, ino, reply)
            }
            _ => self.getattr(req, ino, reply),
        }
    }
}
