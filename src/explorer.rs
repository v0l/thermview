//! Desktop file tree. Directory listings are cached until expanded or refreshed.
use egui::{Align2, Sense, Ui, pos2, vec2};
use egui_bench::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    directory: bool,
}

pub struct Explorer {
    pub root: PathBuf,
    pub selected: Option<PathBuf>,
    expanded: HashSet<PathBuf>,
    listings: HashMap<PathBuf, Result<Vec<Entry>, String>>,
}

impl Explorer {
    pub fn new(root: PathBuf) -> Self {
        Self { root, selected: None, expanded: HashSet::new(), listings: HashMap::new() }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = std::fs::canonicalize(&root).unwrap_or(root);
        self.expanded.clear();
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.listings.clear();
    }

    pub fn reveal(&mut self, path: &Path) {
        let absolute = std::fs::canonicalize(path)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(path));
        let path = absolute.as_path();
        if !path.starts_with(&self.root) {
            if let Some(parent) = path.parent() {
                self.set_root(parent.to_path_buf());
            }
        }
        self.selected = Some(path.to_path_buf());
        let mut parent = path.parent();
        while let Some(dir) = parent {
            if dir == self.root || !dir.starts_with(&self.root) {
                break;
            }
            self.expanded.insert(dir.to_path_buf());
            parent = dir.parent();
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<PathBuf> {
        let root = self.root.clone();
        self.tree(ui, &root, 0)
    }

    fn tree(&mut self, ui: &mut Ui, dir: &Path, depth: usize) -> Option<PathBuf> {
        let listing =
            self.listings.entry(dir.to_path_buf()).or_insert_with(|| read_directory(dir)).clone();
        let entries = match listing {
            Ok(entries) => entries,
            Err(e) => {
                note(ui, &e, FAULT);
                return None;
            }
        };
        if entries.is_empty() {
            hint(ui, "empty folder");
        }
        let mut opened = None;
        for entry in entries {
            let name = entry.path.file_name().unwrap_or_default().to_string_lossy();
            let expanded = self.expanded.contains(&entry.path);
            let selected = self.selected.as_ref() == Some(&entry.path);
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 25.0), Sense::click());
            let p = ui.painter();
            if selected || response.hovered() {
                p.rect_filled(rect, 0.0, if selected { PANEL } else { BAND });
            }
            if selected {
                p.rect_filled(
                    egui::Rect::from_min_size(rect.min, vec2(2.0, rect.height())),
                    0.0,
                    READOUT,
                );
            }
            let x = rect.left() + 8.0 + depth as f32 * 14.0;
            let y = rect.center().y;
            if entry.directory {
                let chevron = if expanded {
                    vec![pos2(x + 1.0, y - 2.0), pos2(x + 5.0, y + 2.0), pos2(x + 9.0, y - 2.0)]
                } else {
                    vec![pos2(x + 3.0, y - 4.0), pos2(x + 7.0, y), pos2(x + 3.0, y + 4.0)]
                };
                p.add(egui::Shape::line(chevron, egui::Stroke::new(1.2, READOUT)));
            } else {
                p.circle_filled(pos2(x + 5.0, y), 1.2, LEGEND);
            }
            p.text(
                pos2(x + 18.0, rect.center().y),
                Align2::LEFT_CENTER,
                &name,
                figure(12.0),
                if selected { READOUT } else { VALUE },
            );
            let response = response.on_hover_text(entry.path.display().to_string());
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::TreeItem, true, selected, name.as_ref())
            });
            if response.clicked() {
                if entry.directory {
                    if expanded {
                        self.expanded.remove(&entry.path);
                    } else {
                        self.expanded.insert(entry.path.clone());
                    }
                } else {
                    self.selected = Some(entry.path.clone());
                    opened = Some(entry.path.clone());
                }
            }
            if entry.directory && self.expanded.contains(&entry.path) {
                if let Some(path) = self.tree(ui, &entry.path, depth + 1) {
                    opened = Some(path);
                }
            }
        }
        opened
    }
}

fn read_directory(dir: &Path) -> Result<Vec<Entry>, String> {
    let read = std::fs::read_dir(dir).map_err(|e| format!("Cannot read folder: {e}"))?;
    let mut entries = Vec::new();
    for entry in read {
        let entry = entry.map_err(|e| format!("Cannot read folder entry: {e}"))?;
        let path = entry.path();
        entries.push(Entry { directory: path.is_dir(), path });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| {
                a.path
                    .file_name()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .cmp(&b.path.file_name().unwrap_or_default().to_ascii_lowercase())
            })
            .then(a.path.cmp(&b.path))
    });
    Ok(entries)
}
