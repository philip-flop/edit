// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use edit::buffer::MoveLineDirection;
use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::input::{kbmod, vk};
use edit::tui::*;
use stdext::arena_format;

use crate::localization::*;
use crate::settings::{CommandSpec, Settings};
use crate::state::*;

pub fn draw_menubar(ctx: &mut Context, state: &mut State) {
    ctx.menubar_begin();
    ctx.attr_background_rgba(state.menubar_color_bg);
    ctx.attr_foreground_rgba(state.menubar_color_fg);
    {
        let contains_focus = ctx.contains_focus();

        if ctx.menubar_menu_begin(loc(LocId::File), 'F') {
            draw_menu_file(ctx, state);
        }
        if !contains_focus && ctx.consume_shortcut(vk::F10) {
            ctx.steal_focus();
        }
        if state.documents.active().is_some() {
            if ctx.menubar_menu_begin(loc(LocId::Edit), 'E') {
                draw_menu_edit(ctx, state);
            }
            if ctx.menubar_menu_begin(loc(LocId::View), 'V') {
                draw_menu_view(ctx, state);
            }
        }
        if ctx.menubar_menu_begin(loc(LocId::Command), 'C') {
            draw_menu_command(ctx, state);
        }
        if Settings::borrow().developer_mode && ctx.menubar_menu_begin(loc(LocId::Debug), 'D') {
            draw_menu_debug(ctx, state);
        }
        if ctx.menubar_menu_begin(loc(LocId::Help), 'H') {
            draw_menu_help(ctx, state);
        }
    }
    ctx.menubar_end();
}

fn draw_menu_file(ctx: &mut Context, state: &mut State) {
    if ctx.menubar_menu_button(loc(LocId::FileNew), 'N', kbmod::CTRL | vk::N) {
        draw_add_untitled_document(ctx, state);
    }
    if ctx.menubar_menu_button(loc(LocId::FileOpen), 'O', kbmod::CTRL | vk::O) {
        state.wants_file_picker = StateFilePicker::Open;
    }
    if state.documents.active().is_some() {
        if ctx.menubar_menu_button(loc(LocId::FileSave), 'S', kbmod::CTRL | vk::S) {
            state.wants_save = true;
        }
        if ctx.menubar_menu_button(loc(LocId::FileSaveAs), 'A', vk::NULL) {
            state.wants_file_picker = StateFilePicker::SaveAs;
        }
    }
    if !Settings::borrow().path.as_os_str().is_empty()
        && ctx.menubar_menu_button(loc(LocId::FilePreferences), 'P', vk::NULL)
    {
        open_settings_file(ctx, state);
    }
    if state.documents.active().is_some()
        && ctx.menubar_menu_button(loc(LocId::FileClose), 'C', kbmod::CTRL | vk::W)
    {
        state.wants_close = true;
    }
    if ctx.menubar_menu_button(loc(LocId::FileExit), 'X', kbmod::CTRL | vk::Q) {
        state.wants_exit = true;
    }
    ctx.menubar_menu_end();
}

fn draw_menu_edit(ctx: &mut Context, state: &mut State) {
    let doc = state.documents.active().unwrap();
    let mut tb = doc.buffer.borrow_mut();

    if ctx.menubar_menu_button(loc(LocId::EditUndo), 'U', kbmod::CTRL | vk::Z) {
        tb.undo();
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditRedo), 'R', kbmod::CTRL | vk::Y) {
        tb.redo();
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditCut), 'T', kbmod::CTRL | vk::X) {
        tb.cut(ctx.clipboard_mut());
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditCopy), 'C', kbmod::CTRL | vk::C) {
        tb.copy(ctx.clipboard_mut());
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditPaste), 'P', kbmod::CTRL | vk::V) {
        tb.paste(ctx.clipboard_ref(), false);
        ctx.needs_rerender();
    }
    if state.wants_search.kind != StateSearchKind::Disabled {
        if ctx.menubar_menu_button(loc(LocId::EditFind), 'F', kbmod::CTRL | vk::F) {
            state.wants_search.kind = StateSearchKind::Search;
            state.wants_search.focus = true;
        }
        if ctx.menubar_menu_button(loc(LocId::EditReplace), 'L', kbmod::CTRL | vk::R) {
            state.wants_search.kind = StateSearchKind::Replace;
            state.wants_search.focus = true;
        }
    }
    if ctx.menubar_menu_button(loc(LocId::EditFindInFiles), 'I', kbmod::CTRL_SHIFT | vk::F) {
        state.wants_project_search = true;
        state.project_search_results = None;
    }
    if ctx.menubar_menu_button(loc(LocId::EditSelectAll), 'A', kbmod::CTRL | vk::A) {
        tb.select_all();
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditMoveLineUp), 'U', kbmod::ALT | vk::UP) {
        tb.move_selected_lines(MoveLineDirection::Up);
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditMoveLineDown), 'D', kbmod::ALT | vk::DOWN) {
        tb.move_selected_lines(MoveLineDirection::Down);
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditDuplicateLine), 'I', kbmod::CTRL | vk::D) {
        tb.duplicate_lines();
        ctx.needs_rerender();
    }
    if ctx.menubar_menu_button(loc(LocId::EditDeleteLine), 'K', kbmod::CTRL_SHIFT | vk::K) {
        tb.delete_lines();
        ctx.needs_rerender();
    }
    if let Some(token) = tb.language().and_then(edit::lsh::line_comment_token) {
        if ctx.menubar_menu_button(loc(LocId::EditToggleComment), 'G', kbmod::CTRL | vk::SLASH) {
            tb.toggle_line_comment(token);
            ctx.needs_rerender();
        }
    }
    ctx.menubar_menu_end();
}

fn draw_menu_view(ctx: &mut Context, state: &mut State) {
    if let Some(doc) = state.documents.active() {
        let mut tb = doc.buffer.borrow_mut();
        let word_wrap = tb.is_word_wrap_enabled();

        // All values on the statusbar are currently document specific.
        if ctx.menubar_menu_button(loc(LocId::ViewFocusStatusbar), 'S', vk::NULL) {
            state.wants_statusbar_focus = true;
        }
        if ctx.menubar_menu_button(loc(LocId::ViewGoToFile), 'F', kbmod::CTRL | vk::P) {
            state.wants_go_to_file = true;
        }
        if ctx.menubar_menu_button(loc(LocId::FileGoto), 'G', kbmod::CTRL | vk::G) {
            state.wants_goto = true;
        }
        if ctx.menubar_menu_checkbox(loc(LocId::ViewWordWrap), 'W', kbmod::ALT | vk::Z, word_wrap) {
            tb.set_word_wrap(!word_wrap);
            ctx.needs_rerender();
        }
    }

    // The menu item toggles visibility. Focusing the pane is bound to the
    // Ctrl+B shortcut instead (handled globally), so no shortcut is shown here.
    if ctx.menubar_menu_checkbox(loc(LocId::ViewFilePane), 'B', vk::NULL, state.file_pane_visible) {
        state.file_pane_visible = !state.file_pane_visible;
        if state.file_pane_visible {
            state.file_pane_focus = true;
        } else {
            state.wants_editor_focus = true;
        }
        ctx.needs_rerender();
    }

    ctx.menubar_menu_end();
}

fn draw_menu_command(ctx: &mut Context, state: &mut State) {
    let commands: Vec<CommandSpec> = Settings::borrow().commands.clone();
    for (i, spec) in commands.iter().enumerate() {
        ctx.next_block_id_mixin(i as u64);
        let shortcut = spec.key.unwrap_or(vk::NULL);
        if ctx.menubar_menu_button(&spec.name, '\0', shortcut) {
            run_command(ctx, state, spec);
        }
    }
    // Always offer a way to edit the command list, even when none are configured.
    if ctx.menubar_menu_button(loc(LocId::CommandUpdate), 'U', vk::NULL) {
        open_settings_file(ctx, state);
    }
    ctx.menubar_menu_end();
}

fn draw_menu_debug(ctx: &mut Context, state: &mut State) {
    if ctx.menubar_menu_button(loc(LocId::DebugShowThemeColors), 'T', vk::NULL) {
        state.wants_theme_colors = true;
    }
    ctx.menubar_menu_end();
}

/// Opens the settings file in a new document, bootstrapping it with the
/// commented template when it is empty. Shared by the File > Preferences
/// item and the Command > Update Commands item.
fn open_settings_file(ctx: &mut Context, state: &mut State) {
    let path = Settings::borrow().path.clone();
    if path.as_os_str().is_empty() {
        return;
    }
    match state.documents.add_file_path(&path) {
        Ok(doc) => {
            Settings::ensure_template_blocks(&mut doc.buffer.borrow_mut());
        }
        Err(err) => error_log_add(ctx, state, err),
    }
}

fn draw_menu_help(ctx: &mut Context, state: &mut State) {
    if ctx.menubar_menu_button(loc(LocId::HelpAbout), 'A', vk::NULL) {
        state.wants_about = true;
    }
    ctx.menubar_menu_end();
}

pub fn draw_dialog_about(ctx: &mut Context, state: &mut State) {
    ctx.modal_begin("about", loc(LocId::AboutDialogTitle));
    {
        ctx.block_begin("content");
        ctx.inherit_focus();
        ctx.attr_padding(Rect::three(1, 2, 1));
        {
            ctx.label("description", "Microsoft Edit");
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.attr_position(Position::Center);

            ctx.label(
                "version",
                &arena_format!(
                    ctx.arena(),
                    "{}{}",
                    loc(LocId::AboutDialogVersion),
                    env!("CARGO_PKG_VERSION")
                ),
            );
            ctx.attr_overflow(Overflow::TruncateHead);
            ctx.attr_position(Position::Center);

            ctx.label("copyright", "Copyright (c) Microsoft Corporation");
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.attr_position(Position::Center);

            ctx.block_begin("choices");
            ctx.inherit_focus();
            ctx.attr_padding(Rect::three(1, 2, 0));
            ctx.attr_position(Position::Center);
            {
                if ctx.button("ok", loc(LocId::Ok), ButtonStyle::default()) {
                    state.wants_about = false;
                }
                ctx.inherit_focus();
            }
            ctx.block_end();
        }
        ctx.block_end();
    }
    if ctx.modal_end() {
        state.wants_about = false;
    }
}

const THEME_COLOR_COUNT: usize = 18;
const THEME_COLOR_NAMES: [&str; THEME_COLOR_COUNT] = [
    "blk", "red", "grn", "ylw", "blu", "mag", "cyn", "wht", "bBlk", "bRed", "bGrn", "bYlw", "bBlu",
    "bMag", "bCyn", "bWht", "bg", "fg",
];

pub fn draw_dialog_theme_colors(ctx: &mut Context, state: &mut State) {
    let width = (ctx.size().width - 6).max(32);
    let height = (ctx.size().height - 6).max(10);

    ctx.modal_begin("theme-colors", loc(LocId::ThemeColorsDialogTitle));
    ctx.attr_intrinsic_size(Size { width, height });
    {
        ctx.block_begin("content");
        ctx.inherit_focus();
        ctx.attr_padding(Rect::three(0, 1, 1));
        {
            ctx.scrollarea_begin("colors", Size { width: 0, height: height - 3 });
            ctx.attr_background_rgba(ctx.indexed_alpha(IndexedColor::Black, 1, 4));
            {
                ctx.table_begin("matrix");
                let mut columns = [7; THEME_COLOR_COUNT + 1];
                columns[0] = 6;
                ctx.table_set_columns(&columns);

                ctx.table_next_row();
                ctx.label("corner", "fg/bg");
                ctx.attr_overflow(Overflow::TruncateTail);
                for bg in 0..THEME_COLOR_COUNT {
                    ctx.next_block_id_mixin(bg as u64);
                    let bg_color = indexed_color(bg);
                    ctx.label("bg", THEME_COLOR_NAMES[bg]);
                    ctx.attr_background_rgba(ctx.indexed(bg_color));
                    ctx.attr_foreground_rgba(ctx.contrasted(ctx.indexed(bg_color)));
                    ctx.attr_overflow(Overflow::TruncateTail);
                }

                for fg in 0..THEME_COLOR_COUNT {
                    ctx.next_block_id_mixin(fg as u64);
                    ctx.table_next_row();
                    let fg_color = indexed_color(fg);
                    ctx.label("fg", THEME_COLOR_NAMES[fg]);
                    ctx.attr_foreground_rgba(ctx.indexed(fg_color));
                    ctx.attr_overflow(Overflow::TruncateTail);

                    for bg in 0..THEME_COLOR_COUNT {
                        ctx.next_block_id_mixin(((fg * THEME_COLOR_COUNT) + bg) as u64);
                        let bg_color = indexed_color(bg);
                        ctx.label("swatch", &arena_format!(ctx.arena(), "{fg:02}/{bg:02}"));
                        ctx.attr_background_rgba(ctx.indexed(bg_color));
                        ctx.attr_foreground_rgba(ctx.indexed(fg_color));
                        ctx.attr_overflow(Overflow::TruncateTail);
                    }
                }
                ctx.table_end();
            }
            ctx.scrollarea_end();

            if ctx.button("ok", loc(LocId::Ok), ButtonStyle::default()) {
                state.wants_theme_colors = false;
            }
            ctx.attr_position(Position::Center);
            ctx.inherit_focus();
        }
        ctx.block_end();
    }
    if ctx.modal_end() {
        state.wants_theme_colors = false;
    }
}

fn indexed_color(index: usize) -> IndexedColor {
    match index {
        0 => IndexedColor::Black,
        1 => IndexedColor::Red,
        2 => IndexedColor::Green,
        3 => IndexedColor::Yellow,
        4 => IndexedColor::Blue,
        5 => IndexedColor::Magenta,
        6 => IndexedColor::Cyan,
        7 => IndexedColor::White,
        8 => IndexedColor::BrightBlack,
        9 => IndexedColor::BrightRed,
        10 => IndexedColor::BrightGreen,
        11 => IndexedColor::BrightYellow,
        12 => IndexedColor::BrightBlue,
        13 => IndexedColor::BrightMagenta,
        14 => IndexedColor::BrightCyan,
        15 => IndexedColor::BrightWhite,
        16 => IndexedColor::Background,
        17 => IndexedColor::Foreground,
        _ => unreachable!(),
    }
}
