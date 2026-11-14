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

    /// `isspace` in the C locale, which is what the game's C runtime uses for `atoi`/`atof`/`sscanf`
    /// (`0x09..=0x0d` and space).
    fn is_c_space(byte: u8) -> bool {
        byte == b' ' || (0x09..=0x0d).contains(&byte)
    }

    /// The decimal path of `BFConfigFile::getInt`/`getIntList` (`FUN_00405218` -> `FUN_00405199` in `zoo.exe`),
    /// which is the classic C runtime `atoi`: skip leading whitespace, take one optional `+`/`-`, accumulate
    /// decimal digits with 32-bit wrapping arithmetic (no overflow check) and stop at the first non-digit.
    /// Text with no leading digits is `0`; trailing text is ignored.
    pub fn c_atoi(text: &str) -> i32 {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() && is_c_space(bytes[i]) {
            i += 1;
        }
        let negative = bytes.get(i) == Some(&b'-');
        if matches!(bytes.get(i), Some(b'-') | Some(b'+')) {
            i += 1;
        }
        let mut value: i32 = 0;
        while let Some(digit) = bytes.get(i).filter(|b| b.is_ascii_digit()) {
            value = value.wrapping_mul(10).wrapping_add(i32::from(digit - b'0'));
            i += 1;
        }
        if negative {
            value.wrapping_neg()
        } else {
            value
        }
    }

    /// The hex path of `BFConfigFile::getInt`/`getIntList`: `sscanf(text, "0x%x", &out)` for a string that starts
    /// with the exact lowercase prefix `0x`. After the literal `0x` the `%x` conversion skips whitespace, takes an
    /// optional sign and an optional second `0x`/`0X`, then reads hex digits with 32-bit wrapping arithmetic.
    /// `None` when no digit follows: `sscanf` then writes nothing, so the caller's value is left untouched.
    fn c_sscanf_hex(after_prefix: &str) -> Option<i32> {
        let bytes = after_prefix.as_bytes();
        let mut i = 0;
        while i < bytes.len() && is_c_space(bytes[i]) {
            i += 1;
        }
        let negative = bytes.get(i) == Some(&b'-');
        if matches!(bytes.get(i), Some(b'-') | Some(b'+')) {
            i += 1;
        }
        if bytes.get(i) == Some(&b'0') && matches!(bytes.get(i + 1), Some(b'x') | Some(b'X')) && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit) {
            i += 2;
        }
        let mut value: u32 = 0;
        let mut any_digit = false;
        while let Some(digit) = bytes.get(i).and_then(|b| (*b as char).to_digit(16)) {
            value = value.wrapping_mul(16).wrapping_add(digit);
            any_digit = true;
            i += 1;
        }
        if !any_digit {
            return None;
        }
        Some(if negative { value.wrapping_neg() } else { value } as i32)
    }

    /// What one real `BFConfigFile::getInt` does to its out-parameter for one stored value string: `Some(v)` when
    /// the game writes `v`, `None` when it writes nothing (a `0x` string with no hex digits after it). Never fails
    /// on non-numeric text: that converts to `0`.
    pub fn vanilla_int(text: &str) -> Option<i32> {
        if text.as_bytes().starts_with(b"0x") {
            c_sscanf_hex(&text[2..])
        } else {
            Some(c_atoi(text))
        }
    }

    /// The string-to-float conversion `BFConfigFile::getFloat`/`getFloatList` use (`FUN_004b40ae`), modelled as the
    /// C runtime's `atof`: skip whitespace, then convert the longest prefix of the form
    /// `[+-]digits[.digits][(e|E)[+-]digits]` (a mantissa needs at least one digit, an exponent needs at least one
    /// digit or it is not consumed). No match is `0.0`. Not modelled, because `FUN_004b3948` was not read: `inf`/`nan`
    /// spellings and a `d`/`D` exponent marker. The game converts through an 80-bit intermediate, so a decimal
    /// string that lands exactly between two doubles can differ in the last bit; the result is then narrowed to `f32`.
    pub fn c_atof(text: &str) -> f64 {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() && is_c_space(bytes[i]) {
            i += 1;
        }
        let start = i;
        if matches!(bytes.get(i), Some(b'-') | Some(b'+')) {
            i += 1;
        }
        let digits_from = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let mut mantissa_digits = i - digits_from;
        if bytes.get(i) == Some(&b'.') {
            let fraction_from = i + 1;
            let mut j = fraction_from;
            while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                j += 1;
            }
            mantissa_digits += j - fraction_from;
            if mantissa_digits > 0 {
                i = j;
            }
        }
        if mantissa_digits == 0 {
            return 0.0;
        }
        if matches!(bytes.get(i), Some(b'e') | Some(b'E')) {
            let mut j = i + 1;
            if matches!(bytes.get(j), Some(b'-') | Some(b'+')) {
                j += 1;
            }
            if bytes.get(j).is_some_and(u8::is_ascii_digit) {
                while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                i = j;
            }
        }
        text[start..i].parse::<f64>().unwrap_or(0.0)
    }

    /// The `Ini`-backed equivalent of one real `BFConfigFile::getInt(section, key, &mut out)`. Returns whether the
    /// call would return true: the section and key exist and the key has at least one non-empty value. On true,
    /// `*out` is assigned exactly as vanilla assigns it - the first value's [`vanilla_int`] result - so text that is
    /// not a number assigns `0`, and a bare `0x` leaves `*out` untouched. On false `*out` is untouched.
    pub fn get_int(ini: &Ini, section: &str, key: &str, out: &mut i32) -> bool {
        let Some(text) = first(ini, section, key) else {
            return false;
        };
        if let Some(value) = vanilla_int(&text) {
            *out = value;
        }
        true
    }

    /// [`get_int`] for `BFConfigFile::getFloat`: on true, `*out` is the first value converted with [`c_atof`] and
    /// narrowed to `f32`.
    pub fn get_float(ini: &Ini, section: &str, key: &str, out: &mut f32) -> bool {
        let Some(text) = first(ini, section, key) else {
            return false;
        };
        *out = c_atof(&text) as f32;
        true
    }

    /// `getInt` for a caller whose out-parameter already holds `default`.
    pub fn int_or(ini: &Ini, section: &str, key: &str, default: i32) -> i32 {
        let mut value = default;
        get_int(ini, section, key, &mut value);
        value
    }

    /// `getFloat` for a caller whose out-parameter already holds `default`.
    pub fn float_or(ini: &Ini, section: &str, key: &str, default: f32) -> f32 {
        let mut value = default;
        get_float(ini, section, key, &mut value);
        value
    }

    /// `BFConfigFile::getIntList`: one element per value line of the key (each value string kept whole, not split
    /// on whitespace), every one converted with [`vanilla_int`]. The result vector starts zeroed in vanilla, so an
    /// element whose conversion writes nothing stays `0`; no element is ever dropped.
    pub fn int_list(ini: &Ini, section: &str, key: &str) -> Vec<i32> {
        values(ini, section, key).iter().map(|text| vanilla_int(text).unwrap_or(0)).collect()
    }

    /// Every whitespace-separated word across every occurrence of `key`, in order. Real vanilla's
    /// `getStringList` returns one element per `key=` line with the whole trimmed value kept intact
    /// (`BFConfigFile::addKeyVal` interns the entire value), so `key = a b c` is a single element there
    /// and three words here. The two agree whenever every value is a single word, which is how the shipped
    /// data this is used on appears to be authored (the live `AMBIENTS_LIVE_GROUP_COMPARE` test compares
    /// against real vanilla). Prefer [`values`] for an exact match and [`int_list`] for integer lists.
    pub fn word_list(ini: &Ini, section: &str, key: &str) -> Vec<String> {
        values(ini, section, key).iter().flat_map(|v| v.split_whitespace().map(str::to_owned)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ini_compat::*;
    use openzt_configparser::ini::Ini;

    fn ini(text: &str) -> Ini {
        let mut ini = Ini::new_cs();
        ini.set_comment_symbols(&[';']);
        ini.read(text.to_string()).expect("test config parses");
        ini
    }

    #[test]
    fn atoi_matches_the_c_runtime() {
        assert_eq!(c_atoi("12"), 12);
        assert_eq!(c_atoi("  -7"), -7);
        assert_eq!(c_atoi("+5"), 5);
        assert_eq!(c_atoi("12abc"), 12);
        assert_eq!(c_atoi("abc"), 0);
        assert_eq!(c_atoi(""), 0);
        assert_eq!(c_atoi("-"), 0);
        assert_eq!(c_atoi("1 2"), 1);
        assert_eq!(c_atoi("0X1F"), 0, "only a lowercase 0x prefix selects the hex path");
        assert_eq!(c_atoi("99999999999"), 1_215_752_191, "accumulates with 32-bit wrapping");
    }

    #[test]
    fn vanilla_int_hex_and_decimal_paths() {
        assert_eq!(vanilla_int("0x1F"), Some(31));
        assert_eq!(vanilla_int("0xff"), Some(255));
        assert_eq!(vanilla_int("0x 1F"), Some(31));
        assert_eq!(vanilla_int("0x0x1F"), Some(31));
        assert_eq!(vanilla_int("0x-1"), Some(-1));
        assert_eq!(vanilla_int("0xFFFFFFFF"), Some(-1));
        assert_eq!(vanilla_int("0x"), None, "sscanf matches nothing, the out-parameter is left alone");
        assert_eq!(vanilla_int("0xZZ"), None);
        assert_eq!(vanilla_int("0X1F"), Some(0));
        assert_eq!(vanilla_int("abc"), Some(0));
        assert_eq!(vanilla_int("42"), Some(42));
    }

    #[test]
    fn atof_takes_the_longest_numeric_prefix() {
        assert_eq!(c_atof("1.5"), 1.5);
        assert_eq!(c_atof("  -2.25e1xyz"), -22.5);
        assert_eq!(c_atof(".5"), 0.5);
        assert_eq!(c_atof("5."), 5.0);
        assert_eq!(c_atof("1.5f"), 1.5);
        assert_eq!(c_atof("1e"), 1.0);
        assert_eq!(c_atof("1e+"), 1.0);
        assert_eq!(c_atof("2E2"), 200.0);
        assert_eq!(c_atof("e5"), 0.0);
        assert_eq!(c_atof("."), 0.0);
        assert_eq!(c_atof("abc"), 0.0);
        assert_eq!(c_atof(""), 0.0);
        assert_eq!(c_atof("0x10"), 0.0, "stops after the leading 0");
    }

    #[test]
    fn get_int_follows_vanilla_found_and_assign_rules() {
        let ini = ini("[a]
plain=12abc
junk=hello
hex=0x1F
bare=0x
empty=
first=1
first=2
");
        let mut out = 99;
        assert!(get_int(&ini, "a", "plain", &mut out));
        assert_eq!(out, 12);
        out = 99;
        assert!(get_int(&ini, "a", "junk", &mut out), "non-numeric text is found and converts to 0");
        assert_eq!(out, 0);
        out = 99;
        assert!(get_int(&ini, "a", "hex", &mut out));
        assert_eq!(out, 31);
        out = 99;
        assert!(get_int(&ini, "a", "bare", &mut out), "found, but sscanf writes nothing");
        assert_eq!(out, 99);
        out = 99;
        assert!(!get_int(&ini, "a", "empty", &mut out), "an empty value is never stored");
        assert_eq!(out, 99);
        assert!(!get_int(&ini, "a", "missing", &mut out));
        assert!(!get_int(&ini, "nosection", "plain", &mut out));
        assert_eq!(out, 99);
        assert!(get_int(&ini, "a", "first", &mut out));
        assert_eq!(out, 1, "a repeated key yields its first value");
        assert_eq!(int_or(&ini, "a", "missing", -1), -1);
        assert_eq!(int_or(&ini, "a", "junk", -1), 0);
    }

    #[test]
    fn get_float_converts_the_numeric_prefix() {
        let ini = ini("[a]
v=2.5e1abc
j=word
first=1.5
first=9
");
        let mut out = 7.0f32;
        assert!(get_float(&ini, "a", "v", &mut out));
        assert_eq!(out, 25.0);
        assert!(get_float(&ini, "a", "j", &mut out));
        assert_eq!(out, 0.0);
        assert!(get_float(&ini, "a", "first", &mut out));
        assert_eq!(out, 1.5);
        out = 7.0;
        assert!(!get_float(&ini, "a", "missing", &mut out));
        assert_eq!(out, 7.0);
        assert_eq!(float_or(&ini, "a", "missing", 3.5), 3.5);
    }

    #[test]
    fn int_list_keeps_one_element_per_value_line() {
        let ini = ini("[a]
v=5
v=oops
v=0x10
v=
v=7 8
");
        assert_eq!(int_list(&ini, "a", "v"), vec![5, 0, 16, 7], "junk is 0, an empty line is dropped, an inline pair stays one element");
        assert!(int_list(&ini, "a", "missing").is_empty());
    }
}
