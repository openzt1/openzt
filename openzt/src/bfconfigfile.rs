//! `BFConfigFile` is the vanilla base class for anything that loads itself from a `.cfg`-style INI
//! block via `getInt`/`getFloat`/`getString`/`getStringList` (e.g. `ZTResearchProgram`,
//! `ZTResearchCategory`, `ZTResearchBranch` - see `resources/decompiles/BFConfigFile_*` and
//! `resources/decompiles/ZTResearch*_load*`). Those classes inherit it directly (`this` gets cast
//! straight to `BFConfigFile*` when calling its methods), so it occupies their first `0xc` bytes.
//! [`BFConfigFile`] itself models only that raw layout - callers that already have a live vanilla
//! object just need to know these bytes aren't theirs to interpret.
//!
//! [`ini_compat`] is the other half: a real reimplementation of `BFConfigFile`'s *parsing* behavior,
//! built on the existing generic `openzt-configparser` `Ini` parser rather than a bespoke port of
//! `BFConfigFile`'s own tree-based engine. It exists so a caller that only ever opens a `.cfg`
//! resource, reads a handful of keys out of it, and throws it away (no other code shares that
//! particular `BFConfigFile` instance) can do the whole thing in Rust - loading through
//! `resource_manager::lazyresourcemap::get_file` instead of a real vanilla `attempt`/`getInt`/
//! `release` call sequence - which also retires the "hand the ctor's tree-root node back to vanilla's
//! shared small-object freelist" dance that pattern otherwise requires (see `ztshowmgr.rs`'s
//! `init_show_params` and `ambients.rs`'s `Ambients::construct` for the pre-existing shape of that
//! dance). First used by `ztresearch.rs`'s `research_config_reimplementation` module for the whole
//! `ZTResearchBranch`/`Category`/`Program` tree; extracted here once `ztshowmgr.rs`/`ambients.rs`
//! needed the identical semantics-matching helpers.
//!
//! This is **not** a drop-in replacement for every `BFConfigFile` use: it only applies where nothing
//! else needs the *object* to keep existing afterward (a config-backed struct another, still-un-ported
//! vanilla caller reads by raw offset must stay real vanilla memory - CLAUDE.md's style-1 constraint -
//! regardless of how the parsing step itself is done).

use std::fmt;
use std::str::FromStr;

use openzt_configparser::ini::Ini;

use crate::{encoding_utils::decode_game_text, resource_manager::lazyresourcemap::get_file};

/// Confirmed via `resources/decompiles/BFConfigFile_BFConfigFile.c`/`_attempt.c`/`_parse.c` and
/// cross-checked against every `ZTResearch*::load*` function that inherits it.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct BFConfigFile {
    tree_root: u32,       // 0x0 - an intrusive rb-tree root holding the parsed config blocks; not meaningful to callers
    loaded: i32,          // 0x4 - "has data" flag, set once `parse`/`addBlock` populates the tree; a freshly-constructed (not yet `attempt`-ed) instance has this at 0
    kind_tag: u8,         // 0x8 - one byte written by `BFConfigFile::BFConfigFile`'s constructor from a caller-supplied parameter (e.g. always 6 for `ZTResearchProgram`); presumably identifies what kind of thing owns this config
    pad_kind_tag: [u8; 3], // 0x9 - the rest of that same word; never written by any code we've seen, so it's leftover/uninitialized allocator memory, not meaningful
}

impl BFConfigFile {
    pub fn is_loaded(&self) -> bool {
        self.loaded != 0
    }

    pub fn kind_tag(&self) -> u8 {
        self.kind_tag
    }
}

impl fmt::Display for BFConfigFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BFConfigFile {{ loaded: {}, kind_tag: {} }}", self.is_loaded(), self.kind_tag)
    }
}

/// `Ini`-backed reimplementation of `BFConfigFile`'s parsing behavior - see the module doc comment for
/// scope and rationale.
pub mod ini_compat {
    use super::*;

    /// Loads and parses a resource-relative `.cfg` path, with vanilla's actual comment convention
    /// (`;` only - `BFConfigFile::parse` never treats `#`/`:` as comments, unlike the leniency
    /// OpenZT's own mod loader allows for `legacy_loading.rs`'s mod `.cfg` files).
    pub fn read_cfg(path: &str) -> Option<Ini> {
        let Some((_, data)) = get_file(path) else {
            tracing::error!("bfconfigfile::ini_compat: resource '{path}' not found");
            return None;
        };
        let text = decode_game_text(&data);
        let mut ini = Ini::new_cs();
        ini.set_comment_symbols(&[';']);
        match ini.read(text) {
            Ok(_) => Some(ini),
            Err(e) => {
                tracing::error!("bfconfigfile::ini_compat: failed to parse '{path}': {e}");
                None
            }
        }
    }

    /// All values for a repeated key, dropping any that trim to empty. Confirmed against real vanilla
    /// `BFConfigFile::addKeyVal` (`BFConfigFile_addKeyVal.c`): a value that trims to nothing is never
    /// pushed onto the key's value vector at all, so from `getString`/`getStringList`'s perspective an
    /// empty `key=` line is indistinguishable from no `key=` line at all - both leave the vector empty.
    /// `Ini::get_vec` keeps the empty string, so this filters it back out to match.
    pub fn values(ini: &Ini, section: &str, key: &str) -> Vec<String> {
        ini.get_vec(section, key).unwrap_or_default().into_iter().filter(|v| !v.trim().is_empty()).collect()
    }

    /// `BFConfigFile::getString`/`getInt`/`getFloat` all return the *first* value for a repeated key
    /// (see `BFConfigFile_getString.c`); `Ini::get` returns the *last* one instead, so pull from
    /// [`values`] directly to match vanilla.
    pub fn first(ini: &Ini, section: &str, key: &str) -> Option<String> {
        values(ini, section, key).into_iter().next()
    }

    /// [`first`], parsed - the `Ini`-backed equivalent of a single real `BFConfigFile::getInt`/
    /// `getFloat` call. Returns `None` (rather than vanilla's "leave the caller's default untouched")
    /// on a missing key or a value that fails to parse - callers use `.unwrap_or(default)` to match.
    pub fn first_parse<T: FromStr>(ini: &Ini, section: &str, key: &str) -> Option<T> {
        first(ini, section, key)?.parse().ok()
    }

    /// The `Ini`-backed equivalent of `BFConfigFile::getStringList`/`getIntList`: every
    /// whitespace-separated word across every occurrence of `key`, in order. Real vanilla's list
    /// getters return one flat array with no line boundary preserved, so this is deliberately
    /// tolerant of either cfg authoring convention - a single `key = a b c` line or three repeated
    /// `key = a`/`key = b`/`key = c` lines both flatten to the same three-word result, matching
    /// whichever one the real file actually uses.
    pub fn word_list(ini: &Ini, section: &str, key: &str) -> Vec<String> {
        values(ini, section, key).iter().flat_map(|v| v.split_whitespace().map(str::to_owned)).collect()
    }
}
