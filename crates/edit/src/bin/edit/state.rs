// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::mem;
use std::path::{Path, PathBuf};

use edit::framebuffer::{DEFAULT_THEME, INDEXED_COLORS_COUNT, IndexedColor};
use edit::helpers::*;
use edit::oklab::StraightRgba;
use edit::tui::*;
use edit::{buffer, icu};
use stdext::string_from_utf8_lossy_owned;

use crate::apperr;
use crate::documents::DocumentManager;
use crate::localization::*;
use crate::settings::CommandSpec;

#[repr(transparent)]
pub struct FormatApperr(apperr::Error);

impl From<apperr::Error> for FormatApperr {
    fn from(err: apperr::Error) -> Self {
        Self(err)
    }
}

impl std::fmt::Display for FormatApperr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            apperr::Error::SettingsInvalid(what) => {
                write!(f, "{}{}", loc(LocId::SettingsInvalid), what)
            }
            apperr::Error::Icu(icu::ICU_MISSING_ERROR) => f.write_str(loc(LocId::ErrorIcuMissing)),
            apperr::Error::Icu(ref err) => err.fmt(f),
            apperr::Error::Io(ref err) => err.fmt(f),
        }
    }
}

pub struct DisplayablePathBuf {
    value: PathBuf,
    str: Cow<'static, str>,
}

impl DisplayablePathBuf {
    #[allow(dead_code, reason = "only used on Windows")]
    pub fn from_string(string: String) -> Self {
        let str = Cow::Borrowed(string.as_str());
        let str = unsafe { mem::transmute::<Cow<'_, str>, Cow<'_, str>>(str) };
        let value = PathBuf::from(string);
        Self { value, str }
    }

    pub fn from_path(value: PathBuf) -> Self {
        let str = value.to_string_lossy();
        let str = unsafe { mem::transmute::<Cow<'_, str>, Cow<'_, str>>(str) };
        Self { value, str }
    }

    pub fn as_path(&self) -> &Path {
        &self.value
    }

    pub fn as_str(&self) -> &str {
        &self.str
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.value.as_os_str().as_encoded_bytes()
    }
}

impl Default for DisplayablePathBuf {
    fn default() -> Self {
        Self { value: Default::default(), str: Cow::Borrowed("") }
    }
}

impl Clone for DisplayablePathBuf {
    fn clone(&self) -> Self {
        Self::from_path(self.value.clone())
    }
}

impl From<OsString> for DisplayablePathBuf {
    fn from(s: OsString) -> Self {
        Self::from_path(PathBuf::from(s))
    }
}

impl<T: ?Sized + AsRef<OsStr>> From<&T> for DisplayablePathBuf {
    fn from(s: &T) -> Self {
        Self::from_path(PathBuf::from(s))
    }
}

/// A single result row for the project-wide "Find in Files" feature.
pub struct ProjectSearchMatch {
    /// Absolute path to the file containing the match.
    pub path: PathBuf,
    /// 1-based line of the match.
    pub line: CoordType,
    /// 1-based column (grapheme) of the match.
    pub column: CoordType,
    /// The `relative/path:line:` location prefix (rendered in an accent color).
    pub location: String,
    /// The matched line's text (rendered in the default foreground color).
    pub text: String,
}

pub struct StateSearch {
    pub kind: StateSearchKind,
    pub focus: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StateSearchKind {
    Hidden,
    Disabled,
    Search,
    Replace,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StateFilePicker {
    None,
    Open,
    SaveAs,

    SaveAsShown, // Transitioned from SaveAs
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StateEncodingChange {
    None,
    Convert,
    Reopen,
}

#[derive(Default)]
pub struct OscTitleFileStatus {
    pub filename: String,
    pub dirty: bool,
}

pub struct State {
    pub menubar_color_bg: StraightRgba,
    pub menubar_color_fg: StraightRgba,

    /// The terminal's own palette, as queried via OSC at startup. Used to
    /// restore colors when the `theme` setting is switched back to `system`.
    pub system_theme: [StraightRgba; INDEXED_COLORS_COUNT],

    pub documents: DocumentManager,

    // A ring buffer of the last 10 errors.
    pub error_log: [String; 10],
    pub error_log_index: usize,
    pub error_log_count: usize,

    pub wants_file_picker: StateFilePicker,
    pub file_picker_pending_dir: DisplayablePathBuf,
    pub file_picker_pending_dir_revision: u64, // Bumped every time `file_picker_pending_dir` changes.
    pub file_picker_pending_name: PathBuf,
    pub file_picker_entries: Option<[Vec<DisplayablePathBuf>; 3]>, // ["..", directories, files]
    pub file_picker_overwrite_warning: Option<PathBuf>,            // The path the warning is about.
    pub file_picker_autocomplete: Vec<DisplayablePathBuf>,

    // Left-hand file browser pane (Yazi-like).
    pub file_pane_visible: bool,
    pub file_pane_focus: bool, // Request to move keyboard focus into the pane.
    pub file_pane_dir: DisplayablePathBuf,
    pub file_pane_dir_revision: u64, // Bumped every time `file_pane_dir` changes.
    pub file_pane_entries: Option<[Vec<DisplayablePathBuf>; 3]>, // ["..", directories, files]
    pub wants_editor_focus: bool,    // Request to move keyboard focus back into the editor.

    pub wants_search: StateSearch,
    pub search_needle: String,
    pub search_replacement: String,
    pub search_options: buffer::SearchOptions,
    pub search_success: bool,

    // Project-wide search (find in files across the working directory).
    pub wants_project_search: bool,
    pub project_search_needle: String,
    pub project_search_options: buffer::SearchOptions,
    pub project_search_results: Option<Vec<ProjectSearchMatch>>,

    pub wants_language_picker: bool,

    pub wants_encoding_picker: bool,
    pub wants_encoding_change: StateEncodingChange,
    pub encoding_picker_needle: String,
    pub encoding_picker_results: Option<Vec<icu::Encoding>>,

    pub wants_save: bool,
    pub wants_statusbar_focus: bool,
    pub wants_indentation_picker: bool,
    pub wants_go_to_file: bool,
    pub wants_about: bool,
    pub wants_theme_colors: bool,
    pub wants_close: bool,

    // Output of the last user-defined command run from the "Command" menu
    // (or its keybinding). See `crate::settings::CommandSpec`.
    pub command_output_visible: bool,
    pub command_output_title: String,
    pub command_output: String,
    pub wants_exit: bool,
    pub wants_goto: bool,
    pub goto_target: String,
    pub goto_invalid: bool,

    pub osc_title_file_status: OscTitleFileStatus,
    pub osc_clipboard_sync: bool,
    pub osc_clipboard_always_send: bool,
    pub exit: bool,
}

impl State {
    pub fn new() -> apperr::Result<Self> {
        Ok(Self {
            menubar_color_bg: StraightRgba::zero(),
            menubar_color_fg: StraightRgba::zero(),
            system_theme: DEFAULT_THEME,

            documents: Default::default(),

            error_log: [const { String::new() }; 10],
            error_log_index: 0,
            error_log_count: 0,

            wants_file_picker: StateFilePicker::None,
            file_picker_pending_dir: Default::default(),
            file_picker_pending_dir_revision: 0,
            file_picker_pending_name: Default::default(),
            file_picker_entries: None,
            file_picker_overwrite_warning: None,
            file_picker_autocomplete: Vec::new(),

            file_pane_visible: false,
            file_pane_focus: false,
            file_pane_dir: Default::default(),
            file_pane_dir_revision: 0,
            file_pane_entries: None,
            wants_editor_focus: false,

            wants_search: StateSearch { kind: StateSearchKind::Hidden, focus: false },
            search_needle: Default::default(),
            search_replacement: Default::default(),
            search_options: Default::default(),
            search_success: true,

            wants_project_search: false,
            project_search_needle: Default::default(),
            project_search_options: Default::default(),
            project_search_results: None,

            wants_language_picker: false,

            wants_encoding_picker: false,
            encoding_picker_needle: Default::default(),
            encoding_picker_results: Default::default(),

            wants_save: false,
            wants_statusbar_focus: false,
            wants_encoding_change: StateEncodingChange::None,
            wants_indentation_picker: false,
            wants_go_to_file: false,
            wants_about: false,
            wants_theme_colors: false,
            wants_close: false,

            command_output_visible: false,
            command_output_title: Default::default(),
            command_output: Default::default(),
            wants_exit: false,
            wants_goto: false,
            goto_target: Default::default(),
            goto_invalid: false,

            osc_title_file_status: Default::default(),
            osc_clipboard_sync: false,
            osc_clipboard_always_send: false,
            exit: false,
        })
    }

    pub fn add_error(&mut self, err: apperr::Error) -> bool {
        let msg = format!("{}", FormatApperr::from(err));
        if msg.is_empty() {
            return false;
        }

        self.error_log[self.error_log_index] = msg;
        self.error_log_index = (self.error_log_index + 1) % self.error_log.len();
        self.error_log_count = self.error_log.len().min(self.error_log_count + 1);
        true
    }

    pub fn active_user_selection_text(&self) -> Option<String> {
        let selection =
            self.documents.active()?.buffer.borrow_mut().extract_user_selection(false)?;
        Some(string_from_utf8_lossy_owned(selection))
    }
}

pub fn draw_add_untitled_document(ctx: &mut Context, state: &mut State) {
    if let Err(err) = state.documents.add_untitled() {
        error_log_add(ctx, state, err);
    }
}

pub fn error_log_add(ctx: &mut Context, state: &mut State, err: apperr::Error) {
    if state.add_error(err) {
        ctx.needs_rerender();
    }
}

pub fn draw_error_log(ctx: &mut Context, state: &mut State) {
    ctx.modal_begin("error", loc(LocId::ErrorDialogTitle));
    ctx.attr_background_rgba(ctx.indexed(IndexedColor::Red));
    ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));
    {
        ctx.block_begin("content");
        ctx.attr_padding(Rect::three(0, 2, 1));
        {
            let off = state.error_log_index + state.error_log.len() - state.error_log_count;

            for i in 0..state.error_log_count {
                let idx = (off + i) % state.error_log.len();
                let msg = &state.error_log[idx][..];

                if !msg.is_empty() {
                    ctx.next_block_id_mixin(i as u64);
                    ctx.label("error", msg);
                    ctx.attr_overflow(Overflow::TruncateTail);
                }
            }
        }
        ctx.block_end();

        if ctx.button("ok", loc(LocId::Ok), ButtonStyle::default()) {
            state.error_log_count = 0;
        }
        ctx.attr_position(Position::Center);
        ctx.inherit_focus();
    }
    if ctx.modal_end() {
        state.error_log_count = 0;
    }
}

/// Runs a user-defined command (see [`CommandSpec`]) via the platform shell,
/// substituting `$FILE` with the active document's path, and shows the
/// captured stdout/stderr/exit code in a scrollable dialog.
pub fn run_command(ctx: &mut Context, state: &mut State, spec: &CommandSpec) {
    let file_path = state
        .documents
        .active()
        .and_then(|doc| doc.path.as_ref())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let command_str = spec.command.replace("$FILE", &file_path);

    #[cfg(windows)]
    let output = std::process::Command::new("cmd.exe").arg("/C").arg(&command_str).output();
    #[cfg(not(windows))]
    let output = std::process::Command::new("sh").arg("-c").arg(&command_str).output();

    let mut text = String::new();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);

            text.push_str(&stdout);
            if !stderr.is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&stderr);
            }
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            match out.status.code() {
                Some(code) => text.push_str(&format!("[exit code: {code}]")),
                None => text.push_str("[terminated by signal]"),
            }
        }
        Err(err) => {
            text.push_str(&format!("Failed to run command: {err}"));
        }
    }

    state.command_output_title = spec.name.clone();
    state.command_output = text;
    state.command_output_visible = true;
    ctx.needs_rerender();
}

pub fn draw_dialog_command_output(ctx: &mut Context, state: &mut State) {
    let width = (ctx.size().width - 20).max(20);
    let height = (ctx.size().height - 10).max(10);

    ctx.modal_begin("command-output", &state.command_output_title);
    ctx.attr_intrinsic_size(Size { width, height });
    {
        ctx.block_begin("content");
        ctx.inherit_focus();
        ctx.attr_padding(Rect::three(0, 1, 1));
        {
            ctx.scrollarea_begin("output", Size { width: 0, height: height - 3 });
            ctx.attr_background_rgba(ctx.indexed_alpha(IndexedColor::Black, 1, 4));
            {
                ctx.list_begin("lines");
                for (i, line) in state.command_output.lines().enumerate() {
                    ctx.next_block_id_mixin(i as u64);
                    ctx.list_item(false, line);
                    ctx.attr_overflow(Overflow::TruncateTail);
                }
                ctx.list_end();
            }
            ctx.scrollarea_end();

            if ctx.button("ok", loc(LocId::Ok), ButtonStyle::default()) {
                state.command_output_visible = false;
            }
            ctx.attr_position(Position::Center);
            ctx.inherit_focus();
        }
        ctx.block_end();
    }
    if ctx.modal_end() {
        state.command_output_visible = false;
    }
}
