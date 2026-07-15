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

use std::{
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::Rect,
    style::{Modifier, Style, Color},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use anyhow::Result;
use indicatif::HumanBytes;
use unicode_width::UnicodeWidthStr;

use crate::common::ArchiveOption;

enum VisibleEntry {
    ParentDir,
    Child { path: PathBuf, is_dir: bool },
    Symlink {
        path: PathBuf,
        target: PathBuf,
        target_is_dir: bool,
    }
}

fn read_dir_entries(dir: &Path) -> io::Result<Vec<(PathBuf, bool, Option<PathBuf>)>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.file_type().is_symlink() {
            let target = fs::read_link(&path).unwrap();
            let target_is_dir = target
                .metadata().ok()
                .map(|m| m.is_dir())
                .unwrap_or(false);
            entries.push((path, target_is_dir, Some(target)));
        } else if meta.is_dir() {
            entries.push((path, true, None));
        } else if meta.is_file() {
            entries.push((path, false, None));
        }
    }
    entries.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            let na = a
                .0
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let nb = b
                .0
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            na.cmp(&nb)
        })
    });
    Ok(entries)
}

fn collect_extension_sizes(root: &Path) -> io::Result<HashMap<String, u64>> {
    let mut ext_sizes = HashMap::new();
    fn walk(dir: &Path, ext_sizes: &mut HashMap<String, u64>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            // using fs::metadata to traverse symlinks
            let ft = fs::metadata(entry.path())?.file_type();
            if ft.is_dir() {
                if let Err(_) = walk(&entry.path(), ext_sizes) {
                    continue;
                }
            } else if ft.is_file() {
                let ext = entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let size = entry.metadata()?.len();
                *ext_sizes.entry(ext).or_insert(0) += size;
            }
        }
        Ok(())
    }
    walk(root, &mut ext_sizes)?;
    Ok(ext_sizes)
}

pub struct ChosenOptions {
    map_paths: HashMap<PathBuf, ArchiveOption>,
    map_exts: HashMap<String, ArchiveOption>,
    root: PathBuf,
}

impl ChosenOptions {
    pub fn effective_option(&self, path: &Path) -> ArchiveOption {
        // first check the direct file path
        if let Some(&opt) = self.map_paths.get(path) {
            return opt;
        }

        // then check extension
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if let Some(&opt) = self.map_exts.get(&ext_lower) {
                return opt;
            }
        }

        // then check parents
        let mut p = path.parent();
        while let Some(curr) = p {
            if let Some(&opt) = self.map_paths.get(curr) {
                return opt;
            }
            if curr == self.root {
                break;
            }
            p = curr.parent();
        }

        // then use the default
        ArchiveOption::default()
    }

    fn save_to_disk(&self) {
        let Some(mut state_dir) = std::env::home_dir() else {
            return;
        };
        state_dir.extend([".local", "state", "archiver"]);
        if let Err(_) = std::fs::create_dir_all(&state_dir) {
            return;
        };
        let mut file = state_dir;
        file.push(format!("{:X}", md5::compute(self.root.as_os_str().as_encoded_bytes())));
        let to_save = (&self.map_paths, &self.map_exts);
        let Ok(bin_data) = postcard::to_allocvec(&to_save) else {
            return;
        };
        if let Err(_) = std::fs::write(file, bin_data) {
            return;
        }
    }

    fn load_from_disk(root_dir: &Path) -> Option<Self> {
        let mut file = std::env::home_dir()?;
        file.extend([".local", "state", "archiver",
            &format!("{:X}", md5::compute(root_dir.as_os_str().as_encoded_bytes()))]);
        let bin_data = std::fs::read(file).ok()?;
        let (map_paths, map_exts) = postcard::from_bytes(&bin_data).ok()?;
        Some(Self { map_paths, map_exts, root: root_dir.to_path_buf() })
    }

    fn new(root_dir: &Path) -> Self {
        Self {
            map_paths: HashMap::new(),
            map_exts: HashMap::new(),
            root: root_dir.to_path_buf()
        }
    }
}

#[derive(PartialEq)]
enum Panel {
    Left,
    Right,
}

struct App {
    root_dir: PathBuf,
    root_name: String,
    current_dir: PathBuf,
    overrides: ChosenOptions,
    visible: Vec<VisibleEntry>,
    list_state: ListState,
    extensions: Vec<(String, u64)>,
    right_state: ListState,
    focus: Panel,
    should_quit: bool,
}

impl App {
    fn new(root_dir: &Path) -> io::Result<Self> {
        let root_name = root_dir
            .file_name()
            .unwrap_or(root_dir.as_os_str())
            .to_string_lossy()
            .into_owned();

        let mut ext_sizes = collect_extension_sizes(root_dir)?;
        let mut extensions: Vec<(String, u64)> = ext_sizes.drain().collect();
        extensions.sort_by(|a, b| b.1.cmp(&a.1));

        let mut app = App {
            root_dir: root_dir.to_path_buf(),
            root_name,
            current_dir: root_dir.to_path_buf(),
            overrides: ChosenOptions::load_from_disk(root_dir)
                .unwrap_or(ChosenOptions::new(root_dir)),
            visible: Vec::new(),
            list_state: ListState::default(),
            extensions,
            right_state: ListState::default(),
            focus: Panel::Left,
            should_quit: false,
        };
        app.update_visible();
        if !app.extensions.is_empty() {
            app.right_state.select(Some(0));
        }
        Ok(app)
    }

    fn reset(&mut self) {
        self.overrides = ChosenOptions::new(&self.root_dir);
    }

    fn selected_index(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    fn update_visible(&mut self) {
        self.visible.clear();

        if self.current_dir != self.root_dir {
            self.visible.push(VisibleEntry::ParentDir);
        }

        if let Ok(children) = read_dir_entries(&self.current_dir) {
            for (path, is_dir, target) in children {
                if let Some(target) = target {
                    self.visible.push(VisibleEntry::Symlink { path, target, target_is_dir: is_dir });
                } else {
                    self.visible.push(VisibleEntry::Child { path, is_dir });
                }
            }
        }

        if self.visible.is_empty() {
            self.list_state.select(None);
        } else {
            let idx = self.selected_index().min(self.visible.len() - 1);
            self.list_state.select(Some(idx));
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            Panel::Left => self.list_state.scroll_down_by(1),
            Panel::Right => self.right_state.scroll_down_by(1),
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            Panel::Left => self.list_state.scroll_up_by(1),
            Panel::Right => self.right_state.scroll_up_by(1),
        }
    }

    fn enter(&mut self) {
        if self.focus != Panel::Left {
            return;
        }
        let idx = self.selected_index();
        if idx >= self.visible.len() {
            return;
        }
        match &self.visible[idx] {
            VisibleEntry::ParentDir => self.go_parent(),
            VisibleEntry::Child { path, is_dir: true } => {
                self.current_dir = path.clone();
                self.update_visible();
            }
            VisibleEntry::Symlink { path, target_is_dir: true, .. } => {
                self.current_dir = path.clone();
                self.update_visible();
            }
            _ => {}
        }
    }

    fn go_parent(&mut self) {
        if self.current_dir != self.root_dir {
            if let Some(parent) = self.current_dir.parent() {
                self.current_dir = parent.to_path_buf();
                self.update_visible();
            }
        }
    }

    fn cycle_option_current(&mut self) {
        if self.focus != Panel::Left {
            return;
        }
        let idx = self.selected_index();
        if idx >= self.visible.len() {
            return;
        }
        match &self.visible[idx] {
            VisibleEntry::ParentDir => {}
            VisibleEntry::Child { path, is_dir } => {
                let effective = self.overrides.effective_option(path);
                let next = effective.next();

                if *is_dir {
                    self.overrides.map_paths
                        .retain(|k, _| !k.strip_prefix(path).is_ok());
                }

                self.overrides.map_paths.insert(path.clone(), next);
            },
            VisibleEntry::Symlink { path, target_is_dir, .. } => {
                let effective = self.overrides.effective_option(path);
                let next = effective.next();

                if *target_is_dir {
                    self.overrides.map_paths
                        .retain(|k, _| !k.strip_prefix(path).is_ok());
                }

                self.overrides.map_paths.insert(path.clone(), next);
            }
        }
    }

    fn cycle_extension_current(&mut self) {
        if self.focus != Panel::Right {
            return;
        }
        let idx = self.right_state.selected().unwrap_or(0);
        if idx < self.extensions.len() {
            let ext = self.extensions[idx].0.clone();
            let current = self.overrides.map_exts.get(&ext).copied().unwrap_or_default();
            self.overrides.map_exts.insert(ext, current.next());
        }
    }
}

fn wrapped_line_count(text: &str, max_width: u16) -> usize {
    let max_width = max_width as usize;
    let mut lines = 0;
    let mut current_line_width = 0;

    for word in text.split(' ') {
        let word_width = word.width();
        if current_line_width + word_width > max_width {
            lines += 1;
            current_line_width = word_width + 1;
        } else {
            current_line_width += word_width + 1;
        }
    }
    if current_line_width > 0 {
        lines += 1;
    }
    lines.max(1)
}

fn ui(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let left_width = (area.width * 4) / 5;
    let right_width = area.width - left_width;
    let left_area = Rect::new(area.x, area.y, left_width, area.height);
    let right_area = Rect::new(area.x + left_width, area.y, right_width, area.height);

    // --- Left Panel ---
    let rel = app.current_dir.strip_prefix(&app.root_dir).unwrap_or(Path::new(""));
    let mut parts = vec![app.root_name.clone()];
    for comp in rel.components() {
        parts.push(comp.as_os_str().to_string_lossy().into_owned());
    }
    let display_path = parts.join("/");
    frame.render_widget(
        ratatui::widgets::Paragraph::new(format!("  Current directory: /{}", display_path))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        Rect::new(left_area.x, left_area.y, left_area.width, 1),
    );

    let items: Vec<ListItem> = app
        .visible
        .iter()
        .map(|ve| match ve {
            VisibleEntry::ParentDir => ListItem::new(Line::from(Span::raw("  .."))),
            VisibleEntry::Child { path, is_dir } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let suffix = if *is_dir { "/" } else { "" };
                let opt_char = app.overrides.effective_option(path).to_char();
                let text = format!("  [{}] {}{}", opt_char, name, suffix);
                ListItem::new(Line::from(Span::raw(text)))
            },
            VisibleEntry::Symlink { path, target, target_is_dir } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let target_str = target.to_string_lossy();
                let suffix = if *target_is_dir { "/" } else { "" };
                let opt_char = app.overrides.effective_option(path).to_char();
                let text = format!("  [{}] {}{} -> {}", opt_char, name, suffix, target_str);
                ListItem::new(Line::from(Span::raw(text)))
            }
        })
        .collect();

    let left_highlight = if app.focus == Panel::Left {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    let list = List::new(items)
        .highlight_style(left_highlight)
        .highlight_symbol("");

    let footer_text = "  q/Ctrl+c:quit  ↑/↓:move  Space:cycle option  Enter/→:descend  ←:parent  r:reset  u:undo reset  Tab:switch active tab  Shift+Enter:confirm choise";
    let footer_paragraph = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true });
    let footer_lines = wrapped_line_count(footer_text, left_area.width);

    let max_footer = left_area.height.saturating_sub(4) as usize;
    let footer_height = footer_lines.min(max_footer).max(1) as u16;

    let rows = left_area.height.saturating_sub(2 + footer_height) as usize;
    let list_area = Rect::new(left_area.x, left_area.y + 2, left_area.width, rows as u16);
    frame.render_stateful_widget(list, list_area, &mut app.list_state);
    frame.render_widget(
        footer_paragraph,
        Rect::new(left_area.x, left_area.y + 2 + rows as u16, left_area.width, footer_height),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Extensions ");
    let inner_right = block.inner(right_area);
    frame.render_widget(block, right_area);

    let ext_items: Vec<ListItem> = app
        .extensions
        .iter()
        .map(|(ext, size)| {
            let opt = app.overrides.map_exts.get(ext).copied().unwrap_or_default();
            let name = if ext.is_empty() { "(none)" } else { ext.as_str() };
            let text = format!("[{}] {}  {:>8}", opt.to_char(), name, HumanBytes(*size));
            ListItem::new(Line::from(Span::raw(text)))
        })
        .collect();

    let right_highlight = if app.focus == Panel::Right {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    let ext_list = List::new(ext_items)
        .highlight_style(right_highlight)
        .highlight_symbol("");

    frame.render_stateful_widget(ext_list, inner_right, &mut app.right_state);
}

pub fn get_choise(terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>, root_dir: &Path) -> Result<ChosenOptions> {
    let mut app = App::new(&root_dir)?;

    while !app.should_quit {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        ratatui::restore();
                        app.overrides.save_to_disk();
                        std::process::exit(0);
                    },
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        ratatui::restore();
                        app.overrides.save_to_disk();
                        std::process::exit(0);
                    },
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.overrides.save_to_disk();
                        app.should_quit = true;
                    },
                    KeyCode::Tab => {
                        app.focus = match app.focus {
                            Panel::Left => Panel::Right,
                            Panel::Right => Panel::Left,
                        };
                    },
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Enter | KeyCode::Right => {
                        if app.focus == Panel::Left {
                            app.enter()
                        }
                    },
                    KeyCode::Left | KeyCode::Char('h') => {
                        if app.focus == Panel::Left {
                            app.go_parent()
                        }
                    },
                    KeyCode::Char(' ') => {
                        match app.focus {
                            Panel::Left => app.cycle_option_current(),
                            Panel::Right => app.cycle_extension_current(),
                        }
                    },
                    KeyCode::Char('r') => app.reset(),
                    _ => {}
                }
            }
        }
    }
    Ok(app.overrides)
}
