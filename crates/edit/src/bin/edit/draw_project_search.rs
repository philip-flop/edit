// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::fs;
use std::path::{Path, PathBuf};

use edit::buffer::{SearchOptions, TextBuffer};
use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::input::vk;
use edit::tui::*;

use stdext::arena_format;

use crate::localization::*;
use crate::state::*;

/// Skip files larger than this to keep the scan responsive and avoid
/// pulling huge blobs into memory.
const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;
/// Cap the number of files we visit so a pathological tree can't hang the UI.
const MAX_FILES_SCANNED: usize = 20_000;
/// Cap the total number of result rows we collect.
const MAX_TOTAL_MATCHES: usize = 5_000;
/// Cap the matches collected from any single file.
const MAX_MATCHES_PER_FILE: usize = 500;

/// Directory names we never descend into (build/VCS noise). This mirrors the
/// spirit of the file pane's ignores while keeping the scan fast.
const IGNORED_DIRS: &[&str] = &[".git", ".hg", ".svn", "node_modules", "target", ".cache"];

pub fn draw_project_search(ctx: &mut Context, state: &mut State) {
    let width = (ctx.size().width - 20).max(20);
    let height = (ctx.size().height - 10).max(10);
    let mut done = false;
    let mut activate: Option<usize> = None;
    // Set when Enter in the query input should move selection to the first result.
    let mut focus_first_result = false;

    // Show the search root in the title, e.g. "Find in Files — /path/to/root".
    let root = search_root(state);
    let title = arena_format!(
        ctx.arena(),
        "{} — {}",
        loc(LocId::ProjectSearchTitle),
        root.to_string_lossy()
    );

    ctx.modal_begin("project-search", &title);
    ctx.attr_intrinsic_size(Size { width, height });
    {
        let contains_focus = ctx.contains_focus();

        // Query input row.
        ctx.table_begin("query");
        ctx.table_set_columns(&[0, COORD_TYPE_SAFE_MAX]);
        ctx.table_set_cell_gap(Size { width: 1, height: 0 });
        ctx.attr_padding(Rect::two(1, 1));
        {
            ctx.table_next_row();
            ctx.label("query-label", loc(LocId::ProjectSearchQueryLabel));

            let changed = ctx.editline("query-input", &mut state.project_search_needle);
            ctx.inherit_focus();
            if changed {
                // Invalidate stale results while the user is typing.
                state.project_search_results = None;
            }

            if ctx.is_focused() && ctx.consume_shortcut(vk::RETURN) {
                run_project_search(state);

                // If the search produced any results, move the selection to the
                // first one so the user can open it with another Enter. Opening
                // is intentionally a separate step.
                if let Some(results) = &state.project_search_results
                    && !results.is_empty()
                {
                    focus_first_result = true;
                }
            }
        }
        ctx.table_end();

        // Results list.
        ctx.scrollarea_begin("results", Size { width: 0, height: height - 4 });
        ctx.attr_background_rgba(ctx.indexed_alpha(IndexedColor::Black, 1, 4));
        {
            ctx.list_begin("result-list");
            ctx.inherit_focus();

            match &state.project_search_results {
                None => {
                    ctx.list_item(false, loc(LocId::ProjectSearchSearching));
                    ctx.attr_overflow(Overflow::TruncateTail);
                }
                Some(results) if results.is_empty() => {
                    ctx.list_item(false, loc(LocId::ProjectSearchNoResults));
                    ctx.attr_overflow(Overflow::TruncateTail);
                }
                Some(results) => {
                    // Render the "path:line:" location in an accent color and the
                    // matched line text in the default foreground color.
                    let accent = ctx.indexed(IndexedColor::BrightCyan);
                    let text_fg = ctx.indexed(IndexedColor::Foreground);

                    for (idx, m) in results.iter().enumerate() {
                        ctx.next_block_id_mixin(idx as u64);
                        ctx.styled_list_item_begin();
                        ctx.attr_overflow(Overflow::TruncateMiddle);

                        ctx.styled_label_set_foreground(accent);
                        ctx.styled_label_add_text(&m.location);
                        ctx.styled_label_set_foreground(text_fg);
                        ctx.styled_label_add_text(&m.text);

                        if ctx.styled_list_item_end(false) == ListSelection::Activated {
                            activate = Some(idx);
                        }

                        // Move selection/focus onto the first result when the
                        // query was just submitted.
                        if idx == 0 && focus_first_result {
                            ctx.list_item_steal_focus();
                        }
                    }
                }
            }

            ctx.list_end();
        }
        ctx.scrollarea_end();

        if contains_focus && ctx.consume_shortcut(vk::ESCAPE) {
            done = true;
        }
    }
    if ctx.modal_end() {
        done = true;
    }

    if let Some(idx) = activate
        && let Some(results) = &state.project_search_results
        && let Some(m) = results.get(idx)
    {
        let path = m.path.clone();
        let point = Point { x: m.column - 1, y: m.line - 1 };
        match state.documents.add_file_path(&path) {
            Ok(doc) => {
                let mut tb = doc.buffer.borrow_mut();
                tb.cursor_move_to_logical(point);
                tb.make_cursor_visible();
                done = true;
            }
            Err(err) => error_log_add(ctx, state, err),
        }
        ctx.needs_rerender();
    }

    if done {
        state.wants_project_search = false;
        state.project_search_results = None;
        ctx.needs_rerender();
    }
}

/// Determines the directory to search from: the file browser root if one is
/// set, otherwise the file picker's working directory.
fn search_root(state: &State) -> PathBuf {
    let pane = state.file_pane_dir.as_path();
    if !pane.as_os_str().is_empty() {
        return pane.to_path_buf();
    }
    state.file_picker_pending_dir.as_path().to_path_buf()
}

/// Walks the working directory and collects matches into `state`.
fn run_project_search(state: &mut State) {
    let needle = state.project_search_needle.clone();
    if needle.is_empty() {
        state.project_search_results = Some(Vec::new());
        return;
    }

    let root = search_root(state);
    let options = state.project_search_options;
    state.project_search_results = Some(scan_directory(&root, &needle, options));
}

/// Recursively walks `root`, searching every eligible text file for `needle`.
/// This is factored out so it can be exercised by unit tests without any
/// terminal UI.
fn scan_directory(root: &Path, needle: &str, options: SearchOptions) -> Vec<ProjectSearchMatch> {
    let mut results = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut files_scanned = 0usize;

    while let Some(dir) = stack.pop() {
        let Ok(iter) = fs::read_dir(&dir) else { continue };

        for entry in iter.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else { continue };

            if metadata.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if IGNORED_DIRS.contains(&name.as_ref()) || name.starts_with('.') {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if !metadata.is_file() || metadata.len() > MAX_FILE_SIZE {
                continue;
            }

            files_scanned += 1;
            if files_scanned > MAX_FILES_SCANNED {
                return results;
            }

            scan_file(root, &path, needle, options, &mut results);

            if results.len() >= MAX_TOTAL_MATCHES {
                return results;
            }
        }
    }

    results
}

/// Reads a single file, skips binary content, and appends its matches.
fn scan_file(
    root: &Path,
    path: &Path,
    needle: &str,
    options: SearchOptions,
    results: &mut Vec<ProjectSearchMatch>,
) {
    let Ok(bytes) = fs::read(path) else { return };

    // Skip files that look binary (contain a NUL byte in the first chunk).
    let probe = &bytes[..bytes.len().min(8192)];
    if probe.contains(&0) {
        return;
    }

    let Ok(mut tb) = TextBuffer::new(false) else { return };
    tb.write_raw(&bytes);

    let matches = match tb.find_all(needle, options, MAX_MATCHES_PER_FILE) {
        Ok(m) => m,
        Err(_) => return,
    };

    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel = rel.to_string_lossy();

    for m in matches {
        results.push(ProjectSearchMatch {
            path: path.to_path_buf(),
            line: m.line,
            column: m.column,
            location: format!("{}:{}:", rel, m.line),
            text: format!(" {}", m.line_text.trim()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, contents: &[u8]) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn scans_tree_and_collects_matches() {
        let root = std::env::temp_dir().join(format!("edit-ps-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        write(&root, "a.txt", b"hello world\nnothing\nhello again\n");
        write(&root, "sub/b.txt", b"say HELLO\n");
        write(&root, ".git/c.txt", b"hello ignored\n");
        write(&root, "bin.dat", b"hello\x00binary\n");

        let mut results = scan_directory(&root, "hello", SearchOptions::default());
        results.sort_by(|a, b| a.location.cmp(&b.location));

        // Two hits in a.txt, one in sub/b.txt (case-insensitive).
        // The .git file is ignored and the binary file is skipped.
        assert_eq!(results.len(), 3, "results: {:?}", results.iter().map(|r| &r.location).collect::<Vec<_>>());
        assert!(results.iter().all(|r| !r.text.contains("ignored")));
        assert!(results.iter().all(|r| !r.text.contains("binary")));
        assert!(results.iter().any(|r| r.location.contains("sub/b.txt") || r.location.contains("sub\\b.txt")));

        // Case-sensitive should drop the uppercase HELLO in sub/b.txt.
        let sensitive = scan_directory(
            &root,
            "hello",
            SearchOptions { match_case: true, ..Default::default() },
        );
        assert_eq!(sensitive.len(), 2);

        let _ = fs::remove_dir_all(&root);
    }
}
