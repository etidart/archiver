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

use std::io;
use std::{fs, path::Path};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::dirbuster::ChosenOptions;
use crate::common::{ArchiveOption, OPTIMIZABLE_EXTS};

fn is_any_optimizable(choise: &ChosenOptions, root: &Path) -> bool {
    fn walk(dir: &Path, choise: &ChosenOptions) -> Option<()> {
        for entry in fs::read_dir(dir).ok()? {
            let entry = entry.ok()?;
            // using fs::metadata to traverse symlinks
            let ft = fs::metadata(entry.path()).ok()?.file_type();
            if ft.is_dir() {
                if let Some(_) = walk(&entry.path(), choise) {
                    return Some(());
                }
            } else if ft.is_file() {
                let ext = entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if let Some(_) = OPTIMIZABLE_EXTS.iter().find(|a| **a == ext) {
                    match choise.effective_option(&entry.path()) {
                        ArchiveOption::Include | ArchiveOption::Compress => return Some(()),
                        _ => (),
                    }
                }
            }
        }
        None
    }
    walk(root, choise).is_some()
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    assert!(percent_x <= 100);
    assert!(percent_y <= 100);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn handle_input(selected_yes: &mut bool) -> Option<bool> {
    if let Ok(event) = event::read() {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                        *selected_yes = !*selected_yes;
                        return None
                    },
                    KeyCode::Enter => return Some(*selected_yes),
                    KeyCode::Char('y') | KeyCode::Char('Y') => return Some(true),
                    KeyCode::Char('n') | KeyCode::Char('N') => return Some(false),
                    KeyCode::Esc => return Some(false),
                    KeyCode::Char('q') => {
                        ratatui::restore();
                        std::process::exit(0);
                    },
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        ratatui::restore();
                        std::process::exit(0);
                    },
                    _ => return None,
                }
            }
        }
    }
    None
}

fn ui(frame: &mut Frame, selected_yes: bool) {
    let area = frame.area();

    let popup_width = 55;
    let popup_height = 7;
    let popup_area = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Optimize Images?")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));
    let inner = block.inner(popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);
    let text_area = layout[0];
    let button_area = layout[1];

    let text = vec![
        Line::from("Some images were found. Do you want to optimize them?"),
        Line::from("It will reduce the images overall size and (possibly) quality."),
        Line::from("This process will take some time."),
    ];

    let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center);
    frame.render_widget(paragraph, text_area);

    let button_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(button_area);

    let yes_area = button_layout[0];
    let no_area = button_layout[1];

    let yes_style = if selected_yes {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let no_style = if !selected_yes {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let yes = Paragraph::new("< Yes >").style(yes_style).alignment(Alignment::Center);
    let no = Paragraph::new("< No >").style(no_style).alignment(Alignment::Center);

    frame.render_widget(yes, yes_area);
    frame.render_widget(no, no_area);
}

pub fn user_wants_optimization(terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>, root_dir: &Path, choise: &ChosenOptions) -> bool {
    if !is_any_optimizable(choise, root_dir) {
        return false;
    }

    let mut selected_yes = false;

    loop {
        let _ = terminal.draw(|f| ui(f, selected_yes));

        if let Some(decision) = handle_input(&mut selected_yes) {
            return decision;
        }
    }
}
