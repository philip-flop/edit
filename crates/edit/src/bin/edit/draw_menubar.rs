// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use edit::buffer::MoveLineDirection;
use edit::framebuffer::{Attributes, IndexedColor};
use edit::helpers::*;
use edit::input::{kbmod, vk};
use edit::tui::*;
use stdext::arena_format;

use crate::localization::*;
use crate::settings::{CommandSpec, Settings, Theme};
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
        state.project_search_needle = state.active_user_selection_text().unwrap_or_default();
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
    if let Some(token) = tb.language().and_then(edit::lsh::line_comment_token)
        && ctx.menubar_menu_button(loc(LocId::EditToggleComment), 'G', kbmod::CTRL | vk::SLASH)
    {
        tb.toggle_line_comment(token);
        ctx.needs_rerender();
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

    if ctx.menubar_submenu_begin(loc(LocId::ViewSetTheme), 'T') {
        draw_menu_theme_choices(ctx);
        ctx.menubar_submenu_end();
    }

    ctx.menubar_menu_end();
}

fn draw_menu_theme_choices(ctx: &mut Context) {
    let current_theme = Settings::borrow().theme;
    for theme in Theme::ALL {
        let label = match theme {
            Theme::System => loc(LocId::ViewThemeSystem),
            Theme::CatppuccinLatte => loc(LocId::ViewThemeCatppuccinLatte),
            Theme::CatppuccinFrappe => loc(LocId::ViewThemeCatppuccinFrappe),
            Theme::CatppuccinMacchiato => loc(LocId::ViewThemeCatppuccinMacchiato),
            Theme::CatppuccinMocha => loc(LocId::ViewThemeCatppuccinMocha),
        };
        if ctx.menubar_menu_checkbox(label, '\0', vk::NULL, current_theme == theme) {
            Settings::set_theme(theme);
            ctx.needs_rerender();
        }
    }
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
            ctx.label("description", "jedit");
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

/// Explicit indexed foreground/background pairs used by editor text and UI.
/// Colors derived through alpha blending or `contrasted()` are intentionally
/// omitted because they are not fixed palette pairs.
const THEME_COLOR_PAIRS: &[(IndexedColor, IndexedColor)] = &[
    (IndexedColor::Foreground, IndexedColor::Background),
    (IndexedColor::Green, IndexedColor::Background),
    (IndexedColor::BrightYellow, IndexedColor::Background),
    (IndexedColor::BrightRed, IndexedColor::Background),
    (IndexedColor::BrightCyan, IndexedColor::Background),
    (IndexedColor::BrightBlue, IndexedColor::Background),
    (IndexedColor::BrightGreen, IndexedColor::Background),
    (IndexedColor::BrightMagenta, IndexedColor::Background),
    (IndexedColor::Black, IndexedColor::White),
    (IndexedColor::BrightWhite, IndexedColor::Red),
    (IndexedColor::BrightWhite, IndexedColor::BrightBlack),
    (IndexedColor::BrightBlack, IndexedColor::BrightWhite),
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
                ctx.table_begin("pairs");
                ctx.table_set_columns(&[10, 10, 16]);
                ctx.table_set_cell_gap(Size { width: 1, height: 0 });

                ctx.table_next_row();
                draw_theme_color_header(ctx, "fg-header", "Foreground");
                draw_theme_color_header(ctx, "bg-header", "Background");
                draw_theme_color_header(ctx, "sample-header", "Sample");

                for (index, &(fg, bg)) in THEME_COLOR_PAIRS.iter().enumerate() {
                    ctx.next_block_id_mixin(index as u64);
                    ctx.table_next_row();
                    ctx.label("fg", indexed_color_name(fg));
                    ctx.attr_overflow(Overflow::TruncateTail);
                    ctx.label("bg", indexed_color_name(bg));
                    ctx.attr_overflow(Overflow::TruncateTail);
                    ctx.label("sample", "Sample text");
                    ctx.attr_background_rgba(ctx.indexed(bg));
                    ctx.attr_foreground_rgba(ctx.indexed(fg));
                    ctx.attr_overflow(Overflow::TruncateTail);
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

fn draw_theme_color_header(ctx: &mut Context, classname: &'static str, text: &str) {
    ctx.block_begin(classname);
    {
        ctx.styled_label_begin("text");
        ctx.styled_label_set_attributes(Attributes::Bold);
        ctx.styled_label_add_text(text);
        ctx.styled_label_end();
        ctx.attr_position(Position::Center);
    }
    ctx.block_end();
}

fn indexed_color_name(color: IndexedColor) -> &'static str {
    match color {
        IndexedColor::Black => "black",
        IndexedColor::Red => "red",
        IndexedColor::Green => "green",
        IndexedColor::Yellow => "yellow",
        IndexedColor::Blue => "blue",
        IndexedColor::Magenta => "magenta",
        IndexedColor::Cyan => "cyan",
        IndexedColor::White => "white",
        IndexedColor::BrightBlack => "brBlack",
        IndexedColor::BrightRed => "brRed",
        IndexedColor::BrightGreen => "brGreen",
        IndexedColor::BrightYellow => "brYellow",
        IndexedColor::BrightBlue => "brBlue",
        IndexedColor::BrightMagenta => "brMagenta",
        IndexedColor::BrightCyan => "brCyan",
        IndexedColor::BrightWhite => "brWhite",
        IndexedColor::Background => "background",
        IndexedColor::Foreground => "foreground",
    }
}
