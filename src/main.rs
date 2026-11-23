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
mod archiver;

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Result;
use clap::Parser;

use crate::{archiver::{create_archive, extract_archive, list_archive}, common::ArchiveOption, dirbuster::get_choice};

#[derive(Debug, clap::Parser)]
#[command(disable_help_flag = true)]
struct Args {
    #[arg(long, action = clap::ArgAction::Help, help = "Print help information")]
    help: Option<bool>,
    #[arg(short = 'h', long = "dereference")]
    resolve_symlinks: bool,
    #[arg(short = 'D', long = "default", default_value = "compress")]
    default_action: ArchiveOption,
    #[arg(short = 'x', long = "extract", conflicts_with = "list")]
    extract: bool,
    #[arg(short = 't', long = "list", conflicts_with = "extract")]
    list: bool,
    #[arg(short = 'C', long = "directory", default_value = ".")]
    directory: PathBuf,
    output_file: PathBuf,
    #[arg(trailing_var_arg = true)]
    paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct Config {
    resolve_symlinks: bool,
    default_action: ArchiveOption,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn main() -> Result<()> {
    let args = Args::parse();
    if args.extract || args.list {
        let paths = args.paths;
        if args.list {
            return list_archive(&args.output_file, &paths);
        }
        return extract_archive(&args.output_file, &args.directory, &paths);
    }

    CONFIG.set(Config {
        resolve_symlinks: args.resolve_symlinks,
        default_action: args.default_action,
    }).unwrap();
    let work_dir = args.paths.first().cloned().unwrap_or_else(|| PathBuf::from("."));
    // Make sure we work with canonical (absolute) paths everywhere.
    let work_dir = work_dir.canonicalize().unwrap_or(work_dir);

    let mut terminal = ratatui::init();

    let dir_actions = get_choice(&mut terminal, &work_dir)?;

    ratatui::restore();

    create_archive(&args.output_file, &work_dir, dir_actions)?;

    Ok(())
}
