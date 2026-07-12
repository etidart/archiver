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

use std::{fs, io, path::{Path, PathBuf}};

use crate::{common::{ArchiveOption, OPTIMIZABLE_EXTS}, dirbuster::ChosenOptions};

fn find_all_optimizable(choise: &ChosenOptions, root: &Path) -> Vec<PathBuf> {
    let mut res = Vec::new();
    fn walk(dir: &Path, choise: &ChosenOptions, res: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            // using fs::metadata to traverse symlinks
            let ft = fs::metadata(entry.path())?.file_type();
            if ft.is_dir() {
                if let Err(_) = walk(&entry.path(), choise, res) {
                    continue;
                }
            } else if ft.is_file() {
                let ext = entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if let Some(_) = OPTIMIZABLE_EXTS.iter().find(|a| **a == ext) {
                    match choise.effective_option(&entry.path()) {
                        ArchiveOption::Include | ArchiveOption::Compress => res.push(entry.path()),
                        _ => (),
                    }
                }
            }
        }
        Ok(())
    }
    let _ = walk(root, choise, &mut res);
    res
}
