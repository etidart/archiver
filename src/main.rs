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

use std::{
    io,
    path::PathBuf,
};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

use crate::dirbuster::get_choise;

#[derive(Debug, clap::Parser)]
struct Args {
    #[arg(default_value = ".")]
    work_dir: PathBuf
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal: Terminal<CrosstermBackend<io::Stdout>> = Terminal::new(backend)?;

    let args = Args::parse();
    let work_dir = args.work_dir;
    // Make sure we work with canonical (absolute) paths everywhere.
    let work_dir = work_dir.canonicalize().unwrap_or(work_dir);

    let dir_actions = get_choise(&mut terminal, &work_dir)?;

    if optimizer_ui::user_wants_optimization(&mut terminal, &work_dir, &dir_actions) {
        todo!(); // optimize
    }

    todo!(); // pack everything

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
