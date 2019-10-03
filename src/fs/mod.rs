mod file;

use fuse::{FileAttr, Filesystem, ReplyAttr, ReplyData, ReplyEntry, Request};
use libc::{EINVAL, EISDIR, ENOENT, ENOTDIR, EREMOTEIO};
use std::convert::TryInto;
use std::ffi::OsStr;
use time::Timespec;

use crate::{requests::RequestMode::*, Comic};
use file::File;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };
const EPOCH: Timespec = Timespec { sec: 0, nsec: 0 };
const GEN: u64 = 0;
const BLOCK_SIZE: u64 = 512;
const DEFAULT_SIZE: u64 = 4096;
const DEFAULT_PERM: u16 = 0o444;

pub struct XkcdFs {
    client: crate::XkcdClient,
}

impl XkcdFs {
    pub fn new(client: crate::XkcdClient) -> Self {
        Self { client }
    }

    const fn blocks(size: u64) -> u64 {
        (size + BLOCK_SIZE - 1) / BLOCK_SIZE
    }

    fn file_attr(&self, request: &Request, file: File) -> Option<FileAttr> {
        info!("Getting attributes for {:?}", file);

        match file {
            File::Root => Some(FileAttr {
                ino: file.inode(),
                size: DEFAULT_SIZE,
                blocks: Self::blocks(DEFAULT_SIZE),
                atime: Timespec::new(0, 0),
                mtime: Timespec::new(0, 0),
                ctime: Timespec::new(0, 0),
                crtime: Timespec::new(0, 0),
                kind: file.filetype(),
                perm: DEFAULT_PERM,
                nlink: 2,
                uid: request.uid(),
                gid: request.gid(),
                rdev: 0,
                flags: 0,
            }),
            File::Image(num) => {
                let comic: Option<Comic> = self.client.request_comic(num, None, VeryFast);
                let image = comic
                    .as_ref()
                    .and_then(|c| self.client.request_rendered_image(&c, None, VeryFast));

                let time = comic.map(|c| c.time()).unwrap_or(EPOCH);

                // Default to std::i64::MAX because some programs interpret
                // file sizes as *signed* integers and don't like values of -1
                let size = image.map(|i| i.len() as u64).unwrap_or(4096);

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
                    nlink: 1,
                    uid: request.uid(),
                    gid: request.gid(),
                    rdev: 0,
                    flags: 0,
                })
            }
            File::MetaFolder(num) => {
                let comic: Option<Comic> = self.client.request_comic(num, None, VeryFast);

                let time = comic.map(|c| c.time()).unwrap_or(EPOCH);

                Some(FileAttr {
                    ino: file.inode(),
                    size: DEFAULT_SIZE,
                    blocks: Self::blocks(DEFAULT_SIZE),
                    atime: time,
                    mtime: time,
                    ctime: time,
                    crtime: time,
                    kind: file.filetype(),
                    perm: DEFAULT_PERM,
                    nlink: 2,
                    uid: request.uid(),
                    gid: request.gid(),
                    rdev: 0,
                    flags: 0,
                })
            }
            File::AltText(num) => {
                let comic: Option<Comic> = self.client.request_comic(num, None, VeryFast);

                let time = comic.as_ref().map(|c| c.time()).unwrap_or(EPOCH);
                let size = comic
                    .as_ref()
                    .map(|c| c.alt.len() as u64)
                    .unwrap_or(DEFAULT_SIZE);

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
                    nlink: 2,
                    uid: request.uid(),
                    gid: request.gid(),
                    rdev: 0,
                    flags: 0,
                })
            }
        }
    }
}

impl<'q> Filesystem for XkcdFs {
    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("Getattr for inode {}", ino);
        let attr = File::from_inode(ino).and_then(|f| self.file_attr(req, f));

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
        let file = match File::from_inode(ino) {
            Some(f @ File::Root) => f,
            Some(f @ File::MetaFolder(_)) => f,
            Some(File::Image(_)) => {
                reply.error(ENOTDIR);
                return;
            }
            Some(File::AltText(_)) => {
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

    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let attr = File::from_inode(parent)
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
        let f = File::from_inode(ino);
        let range_end = offset + size as i64;

        match f {
            Some(File::Image(num)) => {
                debug!("Requesting image file for comic {}", num);

                let comic = self.client.request_comic(num, None, Normal);
                let image =
                    comic.and_then(|c| self.client.request_rendered_image(&c, None, Normal));

                match image {
                    None => {
                        warn!("Could not get image data, returning EREMOTEIO");
                        reply.error(EREMOTEIO)
                    }
                    Some(ref img_data) if offset >= img_data.len() as i64 => {
                        warn!(
                            "Could not index into offset {} with only {} bytes of data, returning EINVAL",
                            offset,
                            img_data.len()
                        );
                        reply.error(EINVAL)
                    }
                    Some(img_data) => {
                        let range_end =
                            std::cmp::min(range_end.try_into().unwrap(), img_data.len());
                        info!(
                            "Got {} bytes of image data, returning bytes {}..{}",
                            img_data.len(),
                            offset,
                            range_end
                        );
                        reply.data(&img_data[offset as usize..range_end]);
                    }
                }
            }
            Some(File::AltText(num)) => {
                debug!("Requesting comic for alt text {}", num);

                let comic = self.client.request_comic(num, None, Normal);

                match comic {
                    None => {
                        warn!("Could not fetch comic {}, returning EREMOTEIO", num);
                        reply.error(EREMOTEIO)
                    }
                    Some(ref comic) if offset >= comic.alt.len() as i64 => {
                        warn!("Could not index into offset {} with only {} bytes of data, returning EINVAL",
                              offset,
                              comic.alt.len());
                        reply.error(EINVAL);
                    }
                    Some(ref comic) => {
                        let bytes = comic.alt.as_bytes();
                        let range_end =
                            std::cmp::min(range_end.try_into().unwrap(), comic.alt.len());

                        reply.data(&bytes[offset as usize..range_end]);
                    }
                }
            }
            Some(f @ File::Root) => {
                warn!("{:?} is a directory, returning EISDIR", f);
                reply.error(EISDIR)
            }
            Some(f @ File::MetaFolder(_)) => {
                warn!("{:?} is a directory, returning EISDIR", f);
                reply.error(EISDIR)
            }
            None => {
                warn!("File does not exist, returning ENOENT");
                reply.error(ENOENT)
            }
        };
    }
}
