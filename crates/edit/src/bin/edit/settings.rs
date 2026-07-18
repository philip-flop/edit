use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use edit::buffer::TextBuffer;
use edit::cell::{Ref, SemiRefCell};
use edit::framebuffer::{
    CATPPUCCIN_FRAPPE, CATPPUCCIN_LATTE, CATPPUCCIN_MACCHIATO, CATPPUCCIN_MOCHA,
    INDEXED_COLORS_COUNT, WIZTERM_DARK,
};
use edit::input::{InputKey, kbmod, vk};
use edit::json;
use edit::lsh::{LANGUAGES, Language};
use edit::oklab::StraightRgba;
use stdext::arena::{read_to_string, scratch_arena};
use stdext::arena_format;

use crate::apperr;

const SETTINGS_HEADER: &[u8] = b"{\n";
const SETTINGS_FOOTER: &[u8] = b"}\n";
const SETTINGS_TEMPLATE_BLOCKS: &[(&[u8], &[u8])] = &[
    (
        b"\"theme\"",
        concat!(
            "    // Color theme. One of: \"system\" (follow the terminal palette),\n",
            "    // \"catppuccin-latte\", \"catppuccin-frappe\", \"catppuccin-macchiato\",\n",
            "    // \"catppuccin-mocha\", \"wizterm-dark\".\n",
            "    // \"theme\": \"system\",\n",
        )
        .as_bytes(),
    ),
    (
        b"\"files.associations\"",
        concat!(
            "    // Maps file name globs to language IDs for syntax highlighting.\n",
            "    // The default is empty (associations are inferred automatically).\n",
            "    // \"files.associations\": {\n",
            "    //     \"*.txt\": \"plaintext\"\n",
            "    // },\n",
        )
        .as_bytes(),
    ),
    (
        b"\"files.trimTrailingWhitespace\"",
        concat!(
            "    // On save, trim trailing whitespace from every line. Default: true.\n",
            "    // \"files.trimTrailingWhitespace\": true,\n",
        )
        .as_bytes(),
    ),
    (
        b"\"files.insertFinalNewline\"",
        concat!(
            "    // On save, ensure the file ends with exactly one newline. Default: true.\n",
            "    // \"files.insertFinalNewline\": true,\n",
        )
        .as_bytes(),
    ),
    (
        b"\"fileBrowser.showAtStartup\"",
        concat!(
            "    // Show the file browser at startup. Default: false.\n",
            "    // \"fileBrowser.showAtStartup\": false,\n",
        )
        .as_bytes(),
    ),
    (
        b"\"commands\"",
        concat!(
            "    // User-defined shell commands, runnable from the \"Command\" menu.\n",
            "    // \"key\" is optional and accepts things like \"F5\" or \"Ctrl+Shift+B\".\n",
            "    // \"$FILE\" in \"command\" is replaced with the current file's path.\n",
            "    // \"commands\": [\n",
            "    //     { \"name\": \"Build\", \"command\": \"cargo build --release\", \"key\": \"F5\" },\n",
            "    //     { \"name\": \"Run gofmt on file\", \"command\": \"gofmt -w $FILE\" }\n",
            "    // ],\n",
        )
        .as_bytes(),
    ),
    (
        b"\"developer.mode\"",
        concat!(
            "    // Enable developer-only diagnostics in the menu bar. Default: false.\n",
            "    // \"developer.mode\": false,\n",
        )
        .as_bytes(),
    ),
];

/// A user-defined shell command, configured via the `commands` array in
/// settings.json and exposed through the "Command" menu (and, optionally,
/// a keybinding).
#[derive(Clone)]
pub struct CommandSpec {
    pub name: String,
    pub command: String,
    pub key: Option<InputKey>,
}

/// Parses a simple `Ctrl+Alt+Shift+<key>` shortcut description, as used by
/// the `key` field of a `commands` entry in settings.json.
fn parse_shortcut(s: &str) -> Option<InputKey> {
    let mut parts = s.split('+').collect::<Vec<_>>();
    let key = parts.pop()?;
    if key.is_empty() {
        return None;
    }

    let mut modifiers = kbmod::NONE;
    for m in parts {
        modifiers |= match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => kbmod::CTRL,
            "alt" => kbmod::ALT,
            "shift" => kbmod::SHIFT,
            "super" | "cmd" | "command" | "win" => kbmod::SUPER,
            _ => return None,
        };
    }

    let base = match key.to_ascii_uppercase().as_str() {
        "F1" => vk::F1,
        "F2" => vk::F2,
        "F3" => vk::F3,
        "F4" => vk::F4,
        "F5" => vk::F5,
        "F6" => vk::F6,
        "F7" => vk::F7,
        "F8" => vk::F8,
        "F9" => vk::F9,
        "F10" => vk::F10,
        "F11" => vk::F11,
        "F12" => vk::F12,
        "TAB" => vk::TAB,
        "ESC" | "ESCAPE" => vk::ESCAPE,
        "ENTER" | "RETURN" => vk::RETURN,
        "SPACE" => vk::SPACE,
        "BACKSPACE" | "BACK" => vk::BACK,
        "DELETE" | "DEL" => vk::DELETE,
        "INSERT" | "INS" => vk::INSERT,
        "HOME" => vk::HOME,
        "END" => vk::END,
        "UP" => vk::UP,
        "DOWN" => vk::DOWN,
        "LEFT" => vk::LEFT,
        "RIGHT" => vk::RIGHT,
        "PAGEUP" | "PRIOR" => vk::PRIOR,
        "PAGEDOWN" | "NEXT" => vk::NEXT,
        "0" => vk::N0,
        "1" => vk::N1,
        "2" => vk::N2,
        "3" => vk::N3,
        "4" => vk::N4,
        "5" => vk::N5,
        "6" => vk::N6,
        "7" => vk::N7,
        "8" => vk::N8,
        "9" => vk::N9,
        "A" => vk::A,
        "B" => vk::B,
        "C" => vk::C,
        "D" => vk::D,
        "E" => vk::E,
        "F" => vk::F,
        "G" => vk::G,
        "H" => vk::H,
        "I" => vk::I,
        "J" => vk::J,
        "K" => vk::K,
        "L" => vk::L,
        "M" => vk::M,
        "N" => vk::N,
        "O" => vk::O,
        "P" => vk::P,
        "Q" => vk::Q,
        "R" => vk::R,
        "S" => vk::S,
        "T" => vk::T,
        "U" => vk::U,
        "V" => vk::V,
        "W" => vk::W,
        "X" => vk::X,
        "Y" => vk::Y,
        "Z" => vk::Z,
        _ => return None,
    };

    Some(base | modifiers)
}

/// The color theme to use for the editor.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Follow the terminal's own color palette (queried via OSC). The default.
    #[default]
    System,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    CatppuccinMocha,
    WizTermDark,
}

impl Theme {
    pub const ALL: [Self; 6] = [
        Self::System,
        Self::CatppuccinLatte,
        Self::CatppuccinFrappe,
        Self::CatppuccinMacchiato,
        Self::CatppuccinMocha,
        Self::WizTermDark,
    ];

    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "system" => Theme::System,
            "catppuccin-latte" => Theme::CatppuccinLatte,
            "catppuccin-frappe" => Theme::CatppuccinFrappe,
            "catppuccin-macchiato" => Theme::CatppuccinMacchiato,
            "catppuccin-mocha" => Theme::CatppuccinMocha,
            "wizterm-dark" => Theme::WizTermDark,
            _ => return None,
        })
    }

    /// The fixed palette for this theme, or `None` when the terminal's
    /// own palette should be used ([`Theme::System`]).
    pub fn palette(self) -> Option<[StraightRgba; INDEXED_COLORS_COUNT]> {
        Some(match self {
            Theme::System => return None,
            Theme::CatppuccinLatte => CATPPUCCIN_LATTE,
            Theme::CatppuccinFrappe => CATPPUCCIN_FRAPPE,
            Theme::CatppuccinMacchiato => CATPPUCCIN_MACCHIATO,
            Theme::CatppuccinMocha => CATPPUCCIN_MOCHA,
            Theme::WizTermDark => WIZTERM_DARK,
        })
    }
}

pub struct Settings {
    pub path: PathBuf,
    pub file_associations: Vec<(String, &'static Language)>,
    pub theme: Theme,
    pub commands: Vec<CommandSpec>,
    /// Trim trailing whitespace from every line on save. Defaults to `true`.
    pub trim_trailing_whitespace: bool,
    /// Ensure the file ends with exactly one final newline on save. Defaults to `true`.
    pub insert_final_newline: bool,
    /// Enables developer-only diagnostic UI. Defaults to `false`.
    pub developer_mode: bool,
    /// Show the file browser at startup on wide terminals. Defaults to `false`.
    pub file_browser_show_at_startup: bool,
}

struct SettingsCell(SemiRefCell<Settings>);
unsafe impl Sync for SettingsCell {}
static SETTINGS: SettingsCell = SettingsCell(SemiRefCell::new(Settings::new()));

/// Set when the settings file is saved from within the editor, so the main
/// loop knows to reload settings and re-apply the theme without a restart.
static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);
static THEME_CHANGE_REQUESTED: AtomicBool = AtomicBool::new(false);

impl Settings {
    /// Fills the given settings.json text buffer with some initial contents for convenience.
    pub fn bootstrap(tb: &mut TextBuffer) {
        tb.set_crlf(false);
        tb.write_raw(SETTINGS_HEADER);
        for (i, &(_, block)) in SETTINGS_TEMPLATE_BLOCKS.iter().enumerate() {
            if i > 0 {
                tb.write_raw(b"\n");
            }
            tb.write_raw(block);
        }
        tb.write_raw(SETTINGS_FOOTER);
        tb.cursor_move_to_logical(Default::default());
        tb.mark_as_clean();
    }

    /// Appends commented template blocks for any known settings that are not
    /// already present in the settings file.
    pub fn ensure_template_blocks(tb: &mut TextBuffer) {
        if tb.text_length() == 0 {
            Self::bootstrap(tb);
            return;
        }

        let text = text_buffer_bytes(tb);
        let mut missing = SETTINGS_TEMPLATE_BLOCKS
            .iter()
            .filter_map(|&(marker, block)| (!byte_contains(&text, marker)).then_some(block))
            .peekable();

        if missing.peek().is_none() {
            return;
        }

        let insert_at = text
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .filter(|&pos| text[pos] == b'}')
            .unwrap_or(text.len());

        tb.cursor_move_to_offset(insert_at);
        let needs_leading_newline = insert_at > 0 && text[insert_at - 1] != b'\n';
        if needs_leading_newline {
            tb.write_raw(b"\n");
        }
        if needs_setting_separator(&text[..insert_at]) {
            tb.write_raw(b",\n");
        }
        for block in missing {
            tb.write_raw(b"\n");
            tb.write_raw(block);
        }
    }

    const fn new() -> Self {
        Settings {
            path: PathBuf::new(),
            file_associations: Vec::new(),
            theme: Theme::System,
            commands: Vec::new(),
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            developer_mode: false,
            file_browser_show_at_startup: false,
        }
    }

    pub fn borrow() -> Ref<'static, Settings> {
        SETTINGS.0.borrow()
    }

    /// If the given path is the settings file, flags a reload for the main loop.
    /// Called after a document is saved, so editing settings.json applies live.
    pub fn note_saved(path: &Path) {
        let is_settings = {
            let s = SETTINGS.0.borrow();
            !s.path.as_os_str().is_empty() && s.path == path
        };
        if is_settings {
            RELOAD_REQUESTED.store(true, Ordering::Relaxed);
        }
    }

    /// Returns and clears a pending reload request. See [`Settings::note_saved`].
    pub fn take_reload_request() -> bool {
        RELOAD_REQUESTED.swap(false, Ordering::Relaxed)
    }

    pub fn set_theme(theme: Theme) {
        let s = &mut *SETTINGS.0.borrow_mut();
        if s.theme != theme {
            s.theme = theme;
            THEME_CHANGE_REQUESTED.store(true, Ordering::Relaxed);
        }
    }

    pub fn take_theme_change_request() -> bool {
        THEME_CHANGE_REQUESTED.swap(false, Ordering::Relaxed)
    }

    pub fn reload() -> apperr::Result<()> {
        let s = &mut *SETTINGS.0.borrow_mut();

        // Reset all members if we had been loaded previously.
        if !s.path.as_os_str().is_empty() {
            *s = Settings::new();
        }

        s.load()
    }

    fn load(&mut self) -> apperr::Result<()> {
        self.path = match settings_json_path() {
            Some(p) => p,
            None => return Ok(()),
        };

        let scratch = scratch_arena(None);
        let str = match read_to_string(&scratch, &self.path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
            Ok(str) => str,
        };
        let Ok(json) = json::parse(&scratch, &str) else {
            return Err(apperr::Error::SettingsInvalid("Invalid JSON"));
        };
        let Some(root) = json.as_object() else {
            return Err(apperr::Error::SettingsInvalid("Non-object root"));
        };

        if let Some(value) = root.get("theme") {
            let Some(id) = value.as_str() else {
                return Err(apperr::Error::SettingsInvalid("theme"));
            };
            let Some(theme) = Theme::from_id(id) else {
                return Err(apperr::Error::SettingsInvalid("theme"));
            };
            self.theme = theme;
        }

        if let Some(value) = root.get("files.trimTrailingWhitespace") {
            let Some(b) = value.as_bool() else {
                return Err(apperr::Error::SettingsInvalid("files.trimTrailingWhitespace"));
            };
            self.trim_trailing_whitespace = b;
        }

        if let Some(value) = root.get("files.insertFinalNewline") {
            let Some(b) = value.as_bool() else {
                return Err(apperr::Error::SettingsInvalid("files.insertFinalNewline"));
            };
            self.insert_final_newline = b;
        }

        if let Some(value) = root.get("developer.mode") {
            let Some(b) = value.as_bool() else {
                return Err(apperr::Error::SettingsInvalid("developer.mode"));
            };
            self.developer_mode = b;
        }

        if let Some(value) = root.get("fileBrowser.showAtStartup") {
            let Some(b) = value.as_bool() else {
                return Err(apperr::Error::SettingsInvalid("fileBrowser.showAtStartup"));
            };
            self.file_browser_show_at_startup = b;
        }

        if let Some(f) = root.get_object("files.associations") {
            for &(mut key, ref value) in f.iter() {
                if !key.contains('/') {
                    key = arena_format!(&*scratch, "**/{key}").leak();
                }

                let Some(id) = value.as_str() else {
                    return Err(apperr::Error::SettingsInvalid("files.associations"));
                };
                let Some(language) = LANGUAGES.iter().find(|lang| lang.id == id) else {
                    return Err(apperr::Error::SettingsInvalid("language ID"));
                };

                self.file_associations.push((key.to_string(), language));
            }
        }

        if let Some(arr) = root.get_array("commands") {
            for item in arr {
                let Some(obj) = item.as_object() else {
                    return Err(apperr::Error::SettingsInvalid("commands"));
                };

                let Some(name) = obj.get_str("name") else {
                    return Err(apperr::Error::SettingsInvalid("commands"));
                };
                let Some(command) = obj.get_str("command") else {
                    return Err(apperr::Error::SettingsInvalid("commands"));
                };

                let key = match obj.get_str("key") {
                    Some(k) => match parse_shortcut(k) {
                        Some(key) => Some(key),
                        None => return Err(apperr::Error::SettingsInvalid("commands.key")),
                    },
                    None => None,
                };

                self.commands.push(CommandSpec {
                    name: name.to_string(),
                    command: command.to_string(),
                    key,
                });
            }
        }

        Ok(())
    }
}

fn text_buffer_bytes(tb: &TextBuffer) -> Vec<u8> {
    let mut text = Vec::with_capacity(tb.text_length());
    let mut off = 0;
    while off < tb.text_length() {
        let chunk = tb.read_forward(off);
        if chunk.is_empty() {
            break;
        }
        text.extend_from_slice(chunk);
        off += chunk.len();
    }
    text
}

fn byte_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn needs_setting_separator(text: &[u8]) -> bool {
    let mut last_significant = None;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut i = 0;

    while i < text.len() {
        let b = text[i];

        if in_line_comment {
            in_line_comment = b != b'\n';
            i += 1;
            continue;
        }

        if in_block_comment {
            in_block_comment = !(b == b'*' && text.get(i + 1) == Some(&b'/'));
            i += if in_block_comment { 1 } else { 2 };
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
                last_significant = Some(b);
            }
            i += 1;
            continue;
        }

        if b == b'/' && text.get(i + 1) == Some(&b'/') {
            in_line_comment = true;
            i += 2;
            continue;
        }

        if b == b'/' && text.get(i + 1) == Some(&b'*') {
            in_block_comment = true;
            i += 2;
            continue;
        }

        if b == b'"' {
            in_string = true;
            last_significant = Some(b);
        } else if !b.is_ascii_whitespace() {
            last_significant = Some(b);
        }
        i += 1;
    }

    !matches!(last_significant, None | Some(b'{') | Some(b','))
}

fn settings_json_path() -> Option<PathBuf> {
    let mut config_dir = config_dir()?;
    config_dir.push("settings.json");
    Some(config_dir)
}

fn config_dir() -> Option<PathBuf> {
    fn var_path(key: &str) -> Option<PathBuf> {
        std::env::var_os(key).map(PathBuf::from)
    }

    fn push(mut path: PathBuf, suffix: &str) -> PathBuf {
        path.push(suffix);
        path
    }

    #[cfg(target_os = "windows")]
    {
        var_path("APPDATA").map(|p| push(p, "JEdit"))
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        var_path("HOME").map(|p| push(p, "Library/Application Support/jedit"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    {
        var_path("XDG_CONFIG_HOME")
            .or_else(|| var_path("HOME").map(|p| push(p, ".config")))
            .map(|p| push(p, "jedit"))
    }
}
