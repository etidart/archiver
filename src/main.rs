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

mod dirbuster;
mod common;
mod optimizer_ui;
mod optimizer;
mod archiver;

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Result;
use clap::Parser;

use crate::{archiver::create_archive, common::ArchiveOption, dirbuster::get_choise};

#[derive(Debug, clap::Parser)]
#[command(disable_help_flag = true)]
struct Args {
    #[arg(long, action = clap::ArgAction::Help, help = "Print help information")]
    help: Option<bool>,
    #[arg(short = 'h', long = "dereference")]
    resolve_symlinks: bool,
    #[arg(short = 'D', long = "default", default_value = "compress")]
    default_action: ArchiveOption,
    output_file: PathBuf,
    #[arg(default_value = ".")]
    work_dir: PathBuf,
}

#[derive(Debug)]
struct Config {
    resolve_symlinks: bool,
    default_action: ArchiveOption,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn main() -> Result<()> {
    let args = Args::parse();
    CONFIG.set(Config {
        resolve_symlinks: args.resolve_symlinks,
        default_action: args.default_action,
    }).unwrap();
    let work_dir = args.work_dir;
    // Make sure we work with canonical (absolute) paths everywhere.
    let work_dir = work_dir.canonicalize().unwrap_or(work_dir);

    let mut terminal = ratatui::init();

    let dir_actions = get_choise(&mut terminal, &work_dir)?;

    let user_wants_opt = optimizer_ui::user_wants_optimization(&mut terminal, &work_dir, &dir_actions);
    ratatui::restore();

    let opt_res = if user_wants_opt {
        optimizer::optimize_images(&work_dir, &dir_actions).ok()
    } else {
        None
    };

    create_archive(&args.output_file, &work_dir, dir_actions, opt_res)?;

    Ok(())
}
