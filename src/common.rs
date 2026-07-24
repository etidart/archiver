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

use std::io::{self, Write};

use crossterm::{cursor::Show, execute};
use serde::{Serialize, Deserialize};
use clap::ValueEnum;

use crate::CONFIG;

pub const OPTIMIZABLE_EXTS: [&str; 3] = ["jpeg", "jpg", "png"];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ValueEnum)]
pub enum ArchiveOption {
    #[value(name = "exclude", alias = "e")]
    Exclude,
    #[value(name = "include", alias = "i")]
    Include,
    #[value(name = "compress", alias = "c")]
    Compress,
}

impl Default for ArchiveOption {
    fn default() -> Self {
        CONFIG.get().unwrap().default_action
    }
}

impl ArchiveOption {
    pub fn next(self) -> Self {
        match self {
            ArchiveOption::Exclude => ArchiveOption::Include,
            ArchiveOption::Include => ArchiveOption::Compress,
            ArchiveOption::Compress => ArchiveOption::Exclude,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            ArchiveOption::Exclude => ' ',
            ArchiveOption::Include => 'I',
            ArchiveOption::Compress => '*',
        }
    }
}

pub fn force_shutdown() {
    ratatui::restore();
    let _ = execute!(io::stdout(), Show);
    let _ = io::stdout().flush();
    std::process::exit(0);
}
