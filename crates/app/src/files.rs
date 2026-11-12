//! The Files grid as shown: every file of the storage is kept here, on the UI
//! thread, and the grid gets the ones matching the search, in the chosen order.
//! Previews and metadata land in both, so a new search keeps them.

use std::cell::RefCell;

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{AppWindow, FileItem};

thread_local! {
    static ALL: RefCell<Vec<FileItem>> = const { RefCell::new(Vec::new()) };
}

/// A new list from Moonraker (newest first).
pub fn set_all(ui: &AppWindow, items: Vec<FileItem>) {
    ALL.with(|all| *all.borrow_mut() = items);
    show(ui);
}

/// Change one file wherever it is: in the full list and, if shown, in the grid.
pub fn update(ui: &AppWindow, path: &str, change: impl Fn(&mut FileItem)) {
    ALL.with(|all| {
        if let Some(item) = all.borrow_mut().iter_mut().find(|f| f.path == path) {
            change(item);
        }
    });
    let files = ui.get_files();
    if let Some(i) =
        (0..files.row_count()).find(|&i| files.row_data(i).is_some_and(|f| f.path == path))
    {
        if let Some(mut item) = files.row_data(i) {
            change(&mut item);
            files.set_row_data(i, item);
        }
    }
}

/// Cards around the first one on the glass that get previews: a row above,
/// this screen and the next two screens.
const LOAD_BEFORE: usize = 3;
const LOAD_AFTER: usize = 15;
/// Previews further than this are dropped (memory: a preview is up to ~200 KB).
const KEEP_BEFORE: usize = 6;
const KEEP_AFTER: usize = 18;

/// The grid scrolled to card `first`: drop previews far from it and return the
/// files near it that still need one, nearest first.
pub fn window(ui: &AppWindow, first: usize) -> Vec<String> {
    let files = ui.get_files();
    let n = files.row_count();
    let keep = first.saturating_sub(KEEP_BEFORE)..(first + KEEP_AFTER).min(n);
    let mut dropped = Vec::new();
    for i in (0..n).filter(|i| !keep.contains(i)) {
        if let Some(mut item) = files.row_data(i) {
            if item.has_thumb {
                item.has_thumb = false;
                item.thumb = Default::default();
                dropped.push(item.path.clone());
                files.set_row_data(i, item);
            }
        }
    }
    if !dropped.is_empty() {
        ALL.with(|all| {
            for item in all
                .borrow_mut()
                .iter_mut()
                .filter(|f| dropped.contains(&f.path))
            {
                item.has_thumb = false;
                item.thumb = Default::default();
            }
        });
    }
    let load = first.saturating_sub(LOAD_BEFORE)..(first + LOAD_AFTER).min(n);
    // The screen first, then below, then the row above.
    let mut order: Vec<usize> = (first.min(n)..load.end).collect();
    order.extend((load.start..first.min(n)).rev());
    order
        .into_iter()
        .filter_map(|i| files.row_data(i))
        .filter(|f| !f.has_thumb)
        .map(|f| f.path.to_string())
        .collect()
}

/// Rebuild the grid with the current search and order; the selection stays.
pub fn show(ui: &AppWindow) {
    let selected: Vec<SharedString> = ui
        .get_files()
        .iter()
        .filter(|f| f.selected)
        .map(|f| f.path)
        .collect();
    let query = ui.get_files_query().to_lowercase();
    let mut rows: Vec<FileItem> = ALL.with(|all| {
        all.borrow()
            .iter()
            .filter(|f| matches(&f.name, &query))
            .cloned()
            .collect()
    });
    for row in &mut rows {
        row.selected = selected.contains(&row.path);
    }
    sort(&mut rows, &ui.get_files_sort());
    ui.set_selected_count(rows.iter().filter(|f| f.selected).count() as i32);
    ui.set_files(ModelRc::new(VecModel::from(rows)));
}

/// Search is a case-insensitive part of the name; `query` is lower case.
fn matches(name: &str, query: &str) -> bool {
    query.trim().is_empty() || name.to_lowercase().contains(query.trim())
}

/// "name" A to Z, "time" shortest print first (unknown last), "size" largest
/// first, anything else newest first.
fn sort(rows: &mut [FileItem], key: &str) {
    match key {
        "name" => rows.sort_by_cached_key(|f| f.name.to_lowercase()),
        "time" => rows.sort_by_key(|f| {
            (
                if f.has_meta && f.minutes > 0 {
                    f.minutes
                } else {
                    i32::MAX
                },
                f.rank,
            )
        }),
        "size" => rows.sort_by_key(|f| (std::cmp::Reverse(f.size_kb), f.rank)),
        _ => rows.sort_by_key(|f| f.rank),
    }
}

/// A file's name to show: no folder, no `.gcode`.
pub fn title(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    [".gcode", ".GCODE", ".gco", ".g"]
        .iter()
        .find_map(|ext| name.strip_suffix(ext))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, rank: i32, minutes: i32, size_kb: i32) -> FileItem {
        FileItem {
            path: name.into(),
            name: name.into(),
            rank,
            minutes,
            has_meta: minutes > 0,
            size_kb,
            ..Default::default()
        }
    }

    fn names(rows: &[FileItem]) -> Vec<&str> {
        rows.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn orders() {
        let mut rows = vec![item("b", 2, 0, 5), item("Ä", 0, 30, 1), item("a", 1, 10, 9)];
        sort(&mut rows, "new");
        assert_eq!(names(&rows), ["Ä", "a", "b"]);
        sort(&mut rows, "name");
        assert_eq!(names(&rows), ["a", "b", "Ä"]);
        sort(&mut rows, "time");
        assert_eq!(names(&rows), ["a", "Ä", "b"]);
        sort(&mut rows, "size");
        assert_eq!(names(&rows), ["a", "b", "Ä"]);
    }

    #[test]
    fn search_and_titles() {
        assert!(matches("Корпус вентилятора", "вент"));
        assert!(matches("Benchy", ""));
        assert!(!matches("Benchy", "куб"));
        assert_eq!(title("parts/Benchy.gcode"), "Benchy");
        assert_eq!(title("Benchy"), "Benchy");
    }
}
