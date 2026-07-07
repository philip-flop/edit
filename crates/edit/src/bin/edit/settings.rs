use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use edit::buffer::TextBuffer;
use edit::cell::{Ref, SemiRefCell};
use edit::framebuffer::{
    CATPPUCCIN_FRAPPE, CATPPUCCIN_LATTE, CATPPUCCIN_MACCHIATO, CATPPUCCIN_MOCHA,
    INDEXED_COLORS_COUNT,
};
use edit::input::{InputKey, kbmod, vk};
use edit::json;
use edit::lsh::{LANGUAGES, Language};
use edit::oklab::StraightRgba;
use stdext::arena::{read_to_string, scratch_arena};
use stdext::arena_format;

use crate::apperr;

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
}

impl Theme {
    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "system" => Theme::System,
            "catppuccin-latte" => Theme::CatppuccinLatte,
            "catppuccin-frappe" => Theme::CatppuccinFrappe,
            "catppuccin-macchiato" => Theme::CatppuccinMacchiato,
            "catppuccin-mocha" => Theme::CatppuccinMocha,
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
}

struct SettingsCell(SemiRefCell<Settings>);
unsafe impl Sync for SettingsCell {}
static SETTINGS: SettingsCell = SettingsCell(SemiRefCell::new(Settings::new()));

/// Set when the settings file is saved from within the editor, so the main
/// loop knows to reload settings and re-apply the theme without a restart.
static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

impl Settings {
    /// Fills the given settings.json text buffer with some initial contents for convenience.
    pub fn bootstrap(tb: &mut TextBuffer) {
        tb.set_crlf(false);
        tb.write_raw(
            concat!(
                "{\n",
                "    // Color theme. One of: \"system\" (follow the terminal palette),\n",
                "    // \"catppuccin-latte\", \"catppuccin-frappe\", \"catppuccin-macchiato\",\n",
                "    // \"catppuccin-mocha\".\n",
                "    // \"theme\": \"system\",\n",
                "\n",
                "    // Maps file name globs to language IDs for syntax highlighting.\n",
                "    // The default is empty (associations are inferred automatically).\n",
                "    // \"files.associations\": {\n",
                "    //     \"*.txt\": \"plaintext\"\n",
                "    // },\n",
                "\n",
                "    // On save, trim trailing whitespace from every line. Default: true.\n",
                "    // \"files.trimTrailingWhitespace\": true,\n",
                "\n",
                "    // On save, ensure the file ends with exactly one newline. Default: true.\n",
                "    // \"files.insertFinalNewline\": true,\n",
                "\n",
                "    // User-defined shell commands, runnable from the \"Command\" menu.\n",
                "    // \"key\" is optional and accepts things like \"F5\" or \"Ctrl+Shift+B\".\n",
                "    // \"$FILE\" in \"command\" is replaced with the current file's path.\n",
                "    // \"commands\": [\n",
                "    //     { \"name\": \"Build\", \"command\": \"cargo build\", \"key\": \"F5\" },\n",
                "    //     { \"name\": \"Run gofmt on file\", \"command\": \"gofmt -w $FILE\" }\n",
                "    // ]\n",
                "}\n",
            )
            .as_bytes(),
        );
        tb.cursor_move_to_logical(Default::default());
        tb.mark_as_clean();
    }

    const fn new() -> Self {
        Settings {
            path: PathBuf::new(),
            file_associations: Vec::new(),
            theme: Theme::System,
            commands: Vec::new(),
            trim_trailing_whitespace: true,
            insert_final_newline: true,
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
        var_path("APPDATA").map(|p| push(p, "Microsoft\\Edit"))
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        var_path("HOME").map(|p| push(p, "Library/Application Support/com.microsoft.edit"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    {
        var_path("XDG_CONFIG_HOME")
            .or_else(|| var_path("HOME").map(|p| push(p, ".config")))
            .map(|p| push(p, "msedit"))
    }
}
