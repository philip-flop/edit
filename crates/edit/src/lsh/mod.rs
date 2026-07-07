// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Microsoft Edit's adapter to LSH.

pub mod cache;
mod definitions;
mod highlighter;

use std::path::Path;

pub use definitions::{FILE_ASSOCIATIONS, HighlightKind, LANGUAGES};
pub use highlighter::*;
pub use lsh::runtime::Language;
use stdext::glob::glob_match;

/// Returns the line-comment token for a language (e.g. `//` or `#`), or `None`
/// if the language has no known single-line comment syntax. Used by the
/// "Toggle line comment" editor command. Language ids match those in the `.lsh`
/// definitions (with `_` replaced by `-`).
pub fn line_comment_token(language: &Language) -> Option<&'static str> {
    match language.id {
        "javascript" | "json" | "lsh" => Some("//"),
        "python" | "shellscript" | "powershell" | "yaml" | "properties" | "ignore"
        | "git-commit" | "git-rebase" => Some("#"),
        // "markdown", "diff", and others have no meaningful line comment: no-op.
        _ => None,
    }
}

pub fn process_file_associations<T>(
    associations: &[(T, &'static Language)],
    path: &Path,
) -> Option<&'static Language>
where
    T: AsRef<[u8]>,
{
    let path = path.as_os_str().as_encoded_bytes();

    for a in associations {
        if glob_match(a.0.as_ref(), path) {
            return Some(a.1);
        }
    }

    None
}
