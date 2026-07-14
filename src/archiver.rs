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

use std::{collections::HashSet, fs::{self, OpenOptions}, io::{self, Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, time::SystemTime};

use anyhow::{Context, Result};
use tar::Header;
use uuid::Uuid;
use xz2::{stream::{Check, Filters, LzmaOptions, MatchFinder, Mode, Stream}, write::XzEncoder};

use crate::{dirbuster::ChosenOptions, optimizer::OptimizerOutput};
use crate::common::ArchiveOption;

struct FileData {
    path: PathBuf,
    mtime: SystemTime,
    size: u64,
}

fn get_lists_of_files(root_dir: &Path, choise: ChosenOptions, optimized: &Option<&HashSet<PathBuf>>) -> (Vec<FileData>, Vec<FileData>) {
    let mut uncompr = Vec::new();
    let mut compr = Vec::new();

    if optimized.is_some() {
        let mut buf = Uuid::encode_buffer();
        let uuid = Uuid::new_v4().hyphenated().encode_lower(&mut buf);
        uncompr.push(FileData { path: PathBuf::from(format!("{}.mkv", uuid)), mtime: SystemTime::now(), size: 0 });
        compr.push(FileData { path: PathBuf::from(format!("{}.csv", uuid)), mtime: SystemTime::now(), size: 0 });
    }

    fn walk(dir: &Path, choise: &ChosenOptions, optimized: &Option<&HashSet<PathBuf>>, uncompr: &mut Vec<FileData>, compr: &mut Vec<FileData>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            // using fs::metadata to traverse symlinks
            let md = fs::metadata(&path)?;
            let ft = md.file_type();
            if ft.is_dir() {
                if let Err(_) = walk(&path, choise, optimized, uncompr, compr) {
                    continue;
                }
            } else if ft.is_file() {
                let is_opt = optimized.as_ref().map(|s| s.contains(&path)).unwrap_or(false);
                if is_opt {
                    continue;
                }

                match choise.effective_option(&path) {
                    ArchiveOption::Include => uncompr.push(FileData { path, mtime: md.modified().unwrap_or(SystemTime::now()), size: md.len() }),
                    ArchiveOption::Compress => compr.push(FileData { path, mtime: md.modified().unwrap_or(SystemTime::now()), size: md.len() }),
                    _ => continue
                }
            }
        }
        Ok(())
    }

    let _ = walk(root_dir, &choise, &optimized, &mut uncompr, &mut compr);

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

fn create_archive(arch_path: &Path, root_dir: &Path, choise: ChosenOptions, optimized: Option<OptimizerOutput>) -> Result<()> {
    let (uncompr, compr) = get_lists_of_files(root_dir, choise, &optimized.as_ref().map(|f| &f.files));

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
    if let Some(opt_items) = optimized.as_ref().map(|f| &f.files) {
        writeln!(&mut list_writer, "optimized")?;
        for opt_el in opt_items {
            writeln!(&mut list_writer, "{}", remove_root(&opt_el, root_dir).display())?;
        }
    }

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



    if let Some(mut opt) = optimized {
        uncompr_header.set_size(0);
        uncompr_header.set_cksum();
        let empty_data = [0u8; 0];
        main_arch.append_data(&mut uncompr_header, "uncompressed.tar", &empty_data[..])?;
        let mut file = main_arch.into_inner()?;

        file.seek(SeekFrom::End(-1024))?;
        let start_pos = file.stream_position()?;

        let mut uncompr_builder = tar::Builder::new(file);
        let mut optimized_vid_header = Header::new_gnu();
        optimized_vid_header.set_mode(0o100777);
        optimized_vid_header.set_uid(0);
        optimized_vid_header.set_gid(0);
        optimized_vid_header.set_mtime(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
        let _ = optimized_vid_header.set_username("root");
        let _ = optimized_vid_header.set_groupname("root");
        optimized_vid_header.set_entry_type(tar::EntryType::Regular);
        let mut opt_vid_writer = uncompr_builder.append_writer(&mut optimized_vid_header, &uncompr[0].path)?;

        io::copy(&mut opt.ffmpeg, &mut opt_vid_writer)?;
        opt_vid_writer.finish()?;

        for uncomp_item in uncompr.iter().skip(1) {
            uncompr_builder.append_path_with_name(&uncomp_item.path, remove_root(&uncomp_item.path, root_dir))?;
        }
        let mut file = uncompr_builder.into_inner()?;
        file.seek(SeekFrom::End(0))?;
        let end_pos = file.stream_position()?;

        let uncompr_len = end_pos.saturating_sub(start_pos);
        assert!(uncompr_len >= 1024);
        file.seek(SeekFrom::Start(start_pos - 512))?;
        let mut header_buf = [0u8; 512];
        file.read_exact(&mut header_buf)?;
        let mut header = Header::from_byte_slice(&header_buf[..]).clone();
        header.set_size(uncompr_len);
        file.seek(SeekFrom::Current(-512))?;
        file.write_all(&header.as_bytes()[..])?;
        file.seek(SeekFrom::End(0))?;
        let mut main_arch = tar::Builder::new(file);

        let compr_writer = main_arch.append_writer(&mut compr_header, "compressed.tar.xz")?;
        let compr_writer = get_xz_encoder(compr_writer);

        let mut compr_builder = tar::Builder::new(compr_writer);

        let csv_data = opt.csv.recv()?;
        let mut csv_header = Header::new_gnu();
        csv_header.set_mode(0o100777);
        csv_header.set_uid(0);
        csv_header.set_gid(0);
        csv_header.set_mtime(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
        let _ = csv_header.set_username("root");
        let _ = csv_header.set_groupname("root");
        csv_header.set_entry_type(tar::EntryType::Regular);
        csv_header.set_size(csv_data.len() as u64);
        csv_header.set_cksum();

        compr_builder.append_data(&mut csv_header, &compr[0].path, &csv_data[..])?;

        for comp_item in compr.iter().skip(1) {
            compr_builder.append_path_with_name(&comp_item.path, remove_root(&comp_item.path, root_dir))?;
        }

        let xz_writer = compr_builder.into_inner()?;
        xz_writer.finish()?;

        main_arch.finish()?;
    } else {
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
    }

    Ok(())
}
