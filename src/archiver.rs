/*
 * Copyright (C) 2026 Arseniy Astankov
 *
 * This file is part of archiver.
 *
 * archiver is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * archiver is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * archiver. If not, see <https://www.gnu.org/licenses/>.
 */

use std::{fs::{self, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}, time::SystemTime};

use anyhow::{Context, Result};
use tar::Header;
use xz2::{stream::{Check, Filters, LzmaOptions, MatchFinder, Mode, Stream}, write::XzEncoder};

use crate::{CONFIG, dirbuster::ChosenOptions};
use crate::common::ArchiveOption;

struct FileData {
    path: PathBuf,
    mtime: SystemTime,
    size: u64,
}

fn get_lists_of_files(root_dir: &Path, choise: ChosenOptions) -> (Vec<FileData>, Vec<FileData>) {
    let mut uncompr = Vec::new();
    let mut compr = Vec::new();

    fn walk(dir: &Path, choise: &ChosenOptions, uncompr: &mut Vec<FileData>, compr: &mut Vec<FileData>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let md = if CONFIG.get().unwrap().resolve_symlinks {
                fs::metadata(&path)
            } else {
                fs::symlink_metadata(&path)
            }?;
            let ft = md.file_type();
            if ft.is_dir() {
                if let Err(_) = walk(&path, choise, uncompr, compr) {
                    continue;
                }
            } else if ft.is_file() {
                match choise.effective_option(&path) {
                    ArchiveOption::Include => uncompr.push(FileData { path, mtime: md.modified().unwrap_or(SystemTime::now()), size: md.len() }),
                    ArchiveOption::Compress => compr.push(FileData { path, mtime: md.modified().unwrap_or(SystemTime::now()), size: md.len() }),
                    _ => continue
                }
            }
        }
        Ok(())
    }

    let _ = walk(root_dir, &choise, &mut uncompr, &mut compr);

    (uncompr, compr)
}

fn remove_root<'a>(path: &'a Path, root: &Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

fn get_xz_encoder<T: io::Write>(output: T) -> XzEncoder<T> {
    let mut options = LzmaOptions::new_preset(9).unwrap();
    options.mode(Mode::Normal)
        .match_finder(MatchFinder::BinaryTree4)
        .dict_size(512 * 1024 * 1024)
        .nice_len(273)
        .depth(10000);

    let mut filters = Filters::new();
    filters.lzma2(&options);

    let stream = Stream::new_stream_encoder(&filters, Check::Crc64).unwrap();
    XzEncoder::new_stream(output, stream)
}

pub fn create_archive(arch_path: &Path, root_dir: &Path, choise: ChosenOptions) -> Result<()> {
    let (uncompr, compr) = get_lists_of_files(root_dir, choise);

    let arch_file = OpenOptions::new().create(true).read(true).write(true).open(arch_path).context("failed to open the output file")?;
    let mut main_arch = tar::Builder::new(arch_file);

    let mut list_header = Header::new_gnu();
    list_header.set_mode(0o100777);
    list_header.set_uid(0);
    list_header.set_gid(0);
    list_header.set_mtime(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let _ = list_header.set_username("root");
    let _ = list_header.set_groupname("root");
    list_header.set_entry_type(tar::EntryType::Regular);

    let mut list_writer = main_arch.append_writer(&mut list_header, "files_list")?;

    if uncompr.len() > 0 {
        writeln!(&mut list_writer, "uncompressed")?;
        for el in &uncompr {
            writeln!(&mut list_writer, "{}\t{}\t{}",
                remove_root(&el.path, root_dir).display(),
                el.mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                el.size,
            )?;
        }
    }

    if compr.len() > 0 {
        writeln!(&mut list_writer, "compressed")?;
        for el in &compr {
            writeln!(&mut list_writer, "{}\t{}\t{}",
                remove_root(&el.path, root_dir).display(),
                el.mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                el.size,
            )?;
        }
    }
    list_writer.finish()?;

    let mut uncompr_header = Header::new_gnu();
    uncompr_header.set_mode(0o100777);
    uncompr_header.set_uid(0);
    uncompr_header.set_gid(0);
    uncompr_header.set_mtime(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let _ = uncompr_header.set_username("root");
    let _ = uncompr_header.set_groupname("root");
    uncompr_header.set_entry_type(tar::EntryType::Regular);

    let mut compr_header = Header::new_gnu();
    compr_header.set_mode(0o100777);
    compr_header.set_uid(0);
    compr_header.set_gid(0);
    compr_header.set_mtime(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let _ = compr_header.set_username("root");
    let _ = compr_header.set_groupname("root");
    compr_header.set_entry_type(tar::EntryType::Regular);

    let uncomp_writer = main_arch.append_writer(&mut uncompr_header, "uncompressed.tar")?;
    let mut uncompr_builder = tar::Builder::new(uncomp_writer);

    for uncomp_item in uncompr.iter() {
        uncompr_builder.append_path_with_name(&uncomp_item.path, remove_root(&uncomp_item.path, root_dir))?;
    }

    uncompr_builder.into_inner()?.finish()?;

    let compr_writer = main_arch.append_writer(&mut compr_header, "compressed.tar.xz")?;
    let compr_writer = get_xz_encoder(compr_writer);

    let mut compr_builder = tar::Builder::new(compr_writer);

    for comp_item in compr.iter() {
        compr_builder.append_path_with_name(&comp_item.path, remove_root(&comp_item.path, root_dir))?;
    }

    let xz_writer = compr_builder.into_inner()?;
    xz_writer.finish()?;

    main_arch.finish()?;

    Ok(())
}
