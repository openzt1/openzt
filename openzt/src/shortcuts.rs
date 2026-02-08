//! Keyboard shortcut registration system.
//!
//! Modules can independently register shortcuts with key+modifier combinations.
//! The HANDLE_KEY_DOWN detour checks matches before calling the original function.
//!
//! # Typestate Pattern
//!
//! Shortcuts use a typestate pattern for type-safe modifier construction.
//! Multiple ergonomic syntaxes are supported:
//!
//! ```rust
//! use openzt::shortcuts::{Key, Ctrl, Shift, Alt, VkKey, R};
//! use windows::Win32::UI::Input::KeyboardAndMouse::*;
//!
//! // Method 1: Using const key items (most ergonomic)
//! let ctrl_r = R + Ctrl;           // Key-first: R key, then Ctrl modifier
//! let ctrl_r_alt = Ctrl + R;       // Modifier-first: Ctrl, then R key
//! let ctrl_shift_r = R + Ctrl + Shift;     // Multiple modifiers
//! let ctrl_shift_r_alt = Ctrl + Shift + R; // Modifier-first with multiple
//!
//! // Method 2: Using VK constants directly
//! let ctrl_a = Key::<{ VK_A.0 as i32 }>::new() + Ctrl;
//!
//! // Method 3: Using VkKey::code() in a const
//! const X_KEY: i32 = VkKey::X.code();
//! let ctrl_x = Key::<X_KEY>::new() + Ctrl;
//!
//! // Register the shortcut
//! register_shortcut("module", "description", ctrl_r, false, || {});
//! ```

use std::marker::PhantomData;
use std::ops::Add;
use std::sync::LazyLock;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

// ============================================================================
// Typestate Pattern for Shortcut Construction
// ============================================================================

/// Marker type for Ctrl modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctrl;

/// Marker type for Alt modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alt;

/// Marker type for Shift modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shift;

/// Convenience constants for nicer syntax
pub const CTRL: Ctrl = Ctrl;
pub const ALT: Alt = Alt;
pub const SHIFT: Shift = Shift;

/// A key with a specific virtual key code.
///
/// This is the starting point for building shortcuts with modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key<const CODE: i32>;

impl<const CODE: i32> Key<CODE> {
    /// Create a new key with the given virtual key code.
    pub const fn new() -> Self {
        Self
    }
}

// ============================================================================
// Const Key Items - for ergonomic shortcut creation
// ============================================================================

// Letters
pub const A: Key<{ VK_A.0 as i32 }> = Key::new();
pub const B: Key<{ VK_B.0 as i32 }> = Key::new();
pub const C: Key<{ VK_C.0 as i32 }> = Key::new();
pub const D: Key<{ VK_D.0 as i32 }> = Key::new();
pub const E: Key<{ VK_E.0 as i32 }> = Key::new();
pub const F: Key<{ VK_F.0 as i32 }> = Key::new();
pub const G: Key<{ VK_G.0 as i32 }> = Key::new();
pub const H: Key<{ VK_H.0 as i32 }> = Key::new();
pub const I: Key<{ VK_I.0 as i32 }> = Key::new();
pub const J: Key<{ VK_J.0 as i32 }> = Key::new();
pub const K: Key<{ VK_K.0 as i32 }> = Key::new();
pub const L: Key<{ VK_L.0 as i32 }> = Key::new();
pub const M: Key<{ VK_M.0 as i32 }> = Key::new();
pub const N: Key<{ VK_N.0 as i32 }> = Key::new();
pub const O: Key<{ VK_O.0 as i32 }> = Key::new();
pub const P: Key<{ VK_P.0 as i32 }> = Key::new();
pub const Q: Key<{ VK_Q.0 as i32 }> = Key::new();
pub const R: Key<{ VK_R.0 as i32 }> = Key::new();
pub const S: Key<{ VK_S.0 as i32 }> = Key::new();
pub const T: Key<{ VK_T.0 as i32 }> = Key::new();
pub const U: Key<{ VK_U.0 as i32 }> = Key::new();
pub const V: Key<{ VK_V.0 as i32 }> = Key::new();
pub const W: Key<{ VK_W.0 as i32 }> = Key::new();
pub const X: Key<{ VK_X.0 as i32 }> = Key::new();
pub const Y: Key<{ VK_Y.0 as i32 }> = Key::new();
pub const Z: Key<{ VK_Z.0 as i32 }> = Key::new();

// Numbers
pub const NUM0: Key<{ VK_0.0 as i32 }> = Key::new();
pub const NUM1: Key<{ VK_1.0 as i32 }> = Key::new();
pub const NUM2: Key<{ VK_2.0 as i32 }> = Key::new();
pub const NUM3: Key<{ VK_3.0 as i32 }> = Key::new();
pub const NUM4: Key<{ VK_4.0 as i32 }> = Key::new();
pub const NUM5: Key<{ VK_5.0 as i32 }> = Key::new();
pub const NUM6: Key<{ VK_6.0 as i32 }> = Key::new();
pub const NUM7: Key<{ VK_7.0 as i32 }> = Key::new();
pub const NUM8: Key<{ VK_8.0 as i32 }> = Key::new();
pub const NUM9: Key<{ VK_9.0 as i32 }> = Key::new();

// Function keys
pub const F1: Key<{ VK_F1.0 as i32 }> = Key::new();
pub const F2: Key<{ VK_F2.0 as i32 }> = Key::new();
pub const F3: Key<{ VK_F3.0 as i32 }> = Key::new();
pub const F4: Key<{ VK_F4.0 as i32 }> = Key::new();
pub const F5: Key<{ VK_F5.0 as i32 }> = Key::new();
pub const F6: Key<{ VK_F6.0 as i32 }> = Key::new();
pub const F7: Key<{ VK_F7.0 as i32 }> = Key::new();
pub const F8: Key<{ VK_F8.0 as i32 }> = Key::new();
pub const F9: Key<{ VK_F9.0 as i32 }> = Key::new();
pub const F10: Key<{ VK_F10.0 as i32 }> = Key::new();
pub const F11: Key<{ VK_F11.0 as i32 }> = Key::new();
pub const F12: Key<{ VK_F12.0 as i32 }> = Key::new();

// Special keys
pub const SPACE: Key<{ VK_SPACE.0 as i32 }> = Key::new();
pub const ENTER: Key<{ VK_RETURN.0 as i32 }> = Key::new();
pub const ESCAPE: Key<{ VK_ESCAPE.0 as i32 }> = Key::new();
pub const TAB: Key<{ VK_TAB.0 as i32 }> = Key::new();
pub const BACKSPACE: Key<{ VK_BACK.0 as i32 }> = Key::new();
pub const INSERT: Key<{ VK_INSERT.0 as i32 }> = Key::new();
pub const DELETE: Key<{ VK_DELETE.0 as i32 }> = Key::new();
pub const HOME: Key<{ VK_HOME.0 as i32 }> = Key::new();
pub const END: Key<{ VK_END.0 as i32 }> = Key::new();
pub const PAGE_UP: Key<{ VK_PRIOR.0 as i32 }> = Key::new();
pub const PAGE_DOWN: Key<{ VK_NEXT.0 as i32 }> = Key::new();
pub const UP: Key<{ VK_UP.0 as i32 }> = Key::new();
pub const DOWN: Key<{ VK_DOWN.0 as i32 }> = Key::new();
pub const LEFT: Key<{ VK_LEFT.0 as i32 }> = Key::new();
pub const RIGHT: Key<{ VK_RIGHT.0 as i32 }> = Key::new();

/// A complete shortcut with type-level modifier states.
///
/// The const generics encode the modifier states at compile time, preventing
/// invalid combinations and enabling efficient matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut<const CTRL: bool, const ALT: bool, const SHIFT: bool, const KEY: i32> {
    _private: PhantomData<()>,
}

impl<const CTRL: bool, const ALT: bool, const SHIFT: bool, const KEY: i32> Shortcut<CTRL, ALT, SHIFT, KEY> {
    const fn new() -> Self {
        Self { _private: PhantomData }
    }

    /// Check if this shortcut matches the given key+modifier state.
    pub fn matches(&self, ctrl: bool, alt: bool, shift: bool, key: i32) -> bool {
        CTRL == ctrl && ALT == alt && SHIFT == shift && KEY == key
    }

    /// Get the key code for this shortcut.
    pub const fn key_code(&self) -> i32 {
        KEY
    }

    /// Get the Ctrl modifier state.
    pub const fn ctrl(&self) -> bool {
        CTRL
    }

    /// Get the Alt modifier state.
    pub const fn alt(&self) -> bool {
        ALT
    }

    /// Get the Shift modifier state.
    pub const fn shift(&self) -> bool {
        SHIFT
    }
}

/// A partial shortcut with modifiers and optionally a key.
///
/// This type cannot be registered as a shortcut - it must be completed
/// by adding modifiers (for Shift-only cases) or a key (for modifier-only cases).
///
/// # Example
///
/// ```rust
/// use openzt::shortcuts::{Ctrl, Shift, R};
///
/// // Shift-only: produces PartialShortcut (not registerable)
/// let partial = R + Shift;  // PartialShortcut<false, false, true, R_CODE>
///
/// // Add Ctrl to make it valid
/// let complete = partial + Ctrl;  // Shortcut<true, false, true, R_CODE>
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartialShortcut<const CTRL: bool, const ALT: bool, const SHIFT: bool, const KEY: i32> {
    _private: PhantomData<()>,
}

impl<const CTRL: bool, const ALT: bool, const SHIFT: bool, const KEY: i32> PartialShortcut<CTRL, ALT, SHIFT, KEY> {
    const fn new() -> Self {
        Self { _private: PhantomData }
    }
}

// ----------------------------------------------------------------------------
// Add implementations for building shortcuts
// ----------------------------------------------------------------------------

// Key + Modifier -> Shortcut
impl<const CODE: i32> Add<Ctrl> for Key<CODE> {
    type Output = Shortcut<true, false, false, CODE>;

    fn add(self, _: Ctrl) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Alt> for Key<CODE> {
    type Output = Shortcut<false, true, false, CODE>;

    fn add(self, _: Alt) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Shift> for Key<CODE> {
    type Output = PartialShortcut<false, false, true, CODE>;

    fn add(self, _: Shift) -> Self::Output {
        PartialShortcut::new()
    }
}

// Shortcut + Modifier (adding modifiers to existing shortcuts)
impl<const ALT: bool, const SHIFT: bool, const KEY: i32> Add<Ctrl> for Shortcut<false, ALT, SHIFT, KEY> {
    type Output = Shortcut<true, ALT, SHIFT, KEY>;

    fn add(self, _: Ctrl) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CTRL: bool, const SHIFT: bool, const KEY: i32> Add<Alt> for Shortcut<CTRL, false, SHIFT, KEY> {
    type Output = Shortcut<CTRL, true, SHIFT, KEY>;

    fn add(self, _: Alt) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CTRL: bool, const ALT: bool, const KEY: i32> Add<Shift> for Shortcut<CTRL, ALT, false, KEY> {
    type Output = Shortcut<CTRL, ALT, true, KEY>;

    fn add(self, _: Shift) -> Self::Output {
        Shortcut::new()
    }
}

// ============================================================================
// Modifier + Modifier -> PartialShortcut
// ============================================================================

impl Add<Alt> for Ctrl {
    type Output = PartialShortcut<true, true, false, -1>;
    fn add(self, _: Alt) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Shift> for Ctrl {
    type Output = PartialShortcut<true, false, true, -1>;
    fn add(self, _: Shift) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Ctrl> for Alt {
    type Output = PartialShortcut<true, true, false, -1>;
    fn add(self, _: Ctrl) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Shift> for Alt {
    type Output = PartialShortcut<false, true, true, -1>;
    fn add(self, _: Shift) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Ctrl> for Shift {
    type Output = PartialShortcut<true, false, true, -1>;
    fn add(self, _: Ctrl) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Alt> for Shift {
    type Output = PartialShortcut<false, true, true, -1>;
    fn add(self, _: Alt) -> Self::Output {
        PartialShortcut::new()
    }
}

// ============================================================================
// PartialShortcut + Key -> Shortcut
// ============================================================================

impl<const CODE: i32> Add<Key<CODE>> for PartialShortcut<true, false, false, -1> {
    type Output = Shortcut<true, false, false, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Key<CODE>> for PartialShortcut<false, true, false, -1> {
    type Output = Shortcut<false, true, false, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

// Note: PartialShortcut<false, false, true, KEY> (Shift-only) should NOT allow adding Key
// since it already has a key. Adding a modifier is required instead.

impl<const CODE: i32> Add<Key<CODE>> for PartialShortcut<true, true, false, -1> {
    type Output = Shortcut<true, true, false, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Key<CODE>> for PartialShortcut<true, false, true, -1> {
    type Output = Shortcut<true, false, true, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Key<CODE>> for PartialShortcut<false, true, true, -1> {
    type Output = Shortcut<false, true, true, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Key<CODE>> for PartialShortcut<true, true, true, -1> {
    type Output = Shortcut<true, true, true, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

// ============================================================================
// PartialShortcut + Modifier -> PartialShortcut (adding third modifier, no key yet)
// ============================================================================

// These impls add a third modifier to PartialShortcuts with KEY=-1 (no key yet)
// Note: Using specific impls instead of generic to avoid conflicts with Shift+Key impls

impl Add<Ctrl> for PartialShortcut<false, true, true, -1> {
    type Output = PartialShortcut<true, true, true, -1>;
    fn add(self, _: Ctrl) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Alt> for PartialShortcut<true, false, true, -1> {
    type Output = PartialShortcut<true, true, true, -1>;
    fn add(self, _: Alt) -> Self::Output {
        PartialShortcut::new()
    }
}

impl Add<Shift> for PartialShortcut<true, true, false, -1> {
    type Output = PartialShortcut<true, true, true, -1>;
    fn add(self, _: Shift) -> Self::Output {
        PartialShortcut::new()
    }
}

// ============================================================================
// Modifier + Key -> Shortcut (single modifier shortcut)
// ============================================================================

impl<const CODE: i32> Add<Key<CODE>> for Ctrl {
    type Output = Shortcut<true, false, false, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Key<CODE>> for Alt {
    type Output = Shortcut<false, true, false, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        Shortcut::new()
    }
}

impl<const CODE: i32> Add<Key<CODE>> for Shift {
    type Output = PartialShortcut<false, false, true, CODE>;
    fn add(self, _: Key<CODE>) -> Self::Output {
        PartialShortcut::new()
    }
}

// ============================================================================
// PartialShortcut + Modifier -> Shortcut (for Shift-only shortcuts)
// ============================================================================

// PartialShortcut<false, false, true, KEY> + Ctrl → Shortcut<true, false, true, KEY>
// Note: These impls work for PartialShortcuts with actual key codes (from Shift + Key)
// They do NOT conflict with the -1 sentinel impls because those are for different modifier combinations

impl<const KEY: i32> Add<Ctrl> for PartialShortcut<false, false, true, KEY> {
    type Output = Shortcut<true, false, true, KEY>;
    fn add(self, _: Ctrl) -> Self::Output {
        Shortcut::new()
    }
}

impl<const KEY: i32> Add<Alt> for PartialShortcut<false, false, true, KEY> {
    type Output = Shortcut<false, true, true, KEY>;
    fn add(self, _: Alt) -> Self::Output {
        Shortcut::new()
    }
}

// ============================================================================
// Shortcut Registry (Runtime storage)
// ============================================================================

/// Internal representation of a registered shortcut.
struct RegisteredShortcut {
    module: String,
    description: String,
    key_code: i32,
    ctrl: bool,
    shift: bool,
    alt: bool,
    callback: fn(),
}

/// Global registry of all registered shortcuts.
static SHORTCUT_REGISTRY: LazyLock<std::sync::Mutex<Vec<RegisteredShortcut>>> = LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// Sealed trait to restrict shortcut registration to typestate shortcuts only.
mod private {
    pub trait SealedShortcut {}
}

impl<const CTRL: bool, const ALT: bool, const SHIFT: bool, const KEY: i32> private::SealedShortcut for Shortcut<CTRL, ALT, SHIFT, KEY> {}

/// Trait for types that can be registered as shortcuts.
///
/// This is sealed to only allow `Shortcut` instances to be registered,
/// preventing runtime errors from invalid combinations.
pub trait Registerable: private::SealedShortcut {
    /// Get the key code.
    fn key_code(&self) -> i32;

    /// Get the Ctrl modifier state.
    fn ctrl(&self) -> bool;

    /// Get the Alt modifier state.
    fn alt(&self) -> bool;

    /// Get the Shift modifier state.
    fn shift(&self) -> bool;
}

impl<const CTRL: bool, const ALT: bool, const SHIFT: bool, const KEY: i32> Registerable for Shortcut<CTRL, ALT, SHIFT, KEY> {
    fn key_code(&self) -> i32 {
        KEY
    }

    fn ctrl(&self) -> bool {
        CTRL
    }

    fn alt(&self) -> bool {
        ALT
    }

    fn shift(&self) -> bool {
        SHIFT
    }
}

/// Register a keyboard shortcut.
///
/// # Arguments
///
/// * `module` - Name of the module registering the shortcut (for logging/debugging)
/// * `description` - Human-readable description of what the shortcut does
/// * `shortcut` - The shortcut definition (built using typestate pattern)
/// * `override_existing` - If true, replace existing shortcuts with same key+modifiers
/// * `callback` - Function to call when shortcut is triggered (runs on game thread)
///
/// # Returns
///
/// * `Ok(())` - Successfully registered
/// * `Err(String)` - Shortcut already registered and override was false
///
/// # Example
///
/// ```rust
/// use openzt::shortcuts::{Key, Ctrl, Shift};
///
/// register_shortcut(
///     "mymodule",
///     "Do something cool",
///     Key::<{ VK_A.0 as i32 }>::new() + Ctrl + Shift,
///     false,
///     || {
///         // Handler code
///     }
/// )?;
/// ```
pub fn register_shortcut<S: Registerable>(module: &str, description: &str, shortcut: S, override_existing: bool, callback: fn()) -> Result<(), String> {
    let key_code = shortcut.key_code();
    let ctrl = shortcut.ctrl();
    let alt = shortcut.alt();
    let shift = shortcut.shift();

    let mut registry = SHORTCUT_REGISTRY.lock().unwrap();

    // Check for existing shortcut with same key+modifiers
    let existing_idx = registry
        .iter()
        .position(|s| s.key_code == key_code && s.ctrl == ctrl && s.shift == shift && s.alt == alt);

    match (existing_idx, override_existing) {
        (Some(idx), true) => {
            tracing::warn!("Shortcut override: {} replacing {} for key combo", module, registry[idx].module);
            registry[idx] = RegisteredShortcut {
                module: module.to_string(),
                description: description.to_string(),
                key_code,
                ctrl,
                shift,
                alt,
                callback,
            };
            Ok(())
        }
        (Some(_idx), false) => Err("Shortcut already registered (use override=true to replace)".to_string()),
        (None, _) => {
            registry.push(RegisteredShortcut {
                module: module.to_string(),
                description: description.to_string(),
                key_code,
                ctrl,
                shift,
                alt,
                callback,
            });
            tracing::debug!(
                "Shortcut registered: {} - {} (Ctrl+Shift+Alt: {}{}{} key: {:#x})",
                module,
                description,
                ctrl,
                shift,
                alt,
                key_code
            );
            Ok(())
        }
    }
}

/// Check if a key press matches any registered shortcuts.
///
/// Returns the callback function if a match is found, None otherwise.
pub fn check_shortcuts(key_code: i32) -> Option<fn()> {
    // GetAsyncKeyState returns i16, check high bit (bit 15) for key state
    let ctrl_pressed = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 } & 0x8000 != 0;
    let shift_pressed = unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 } & 0x8000 != 0;
    let alt_pressed = unsafe { GetAsyncKeyState(VK_MENU.0 as i32) as u16 } & 0x8000 != 0;

    let registry = SHORTCUT_REGISTRY.lock().unwrap();
    registry
        .iter()
        .find(|s| s.key_code == key_code && s.ctrl == ctrl_pressed && s.shift == shift_pressed && s.alt == alt_pressed)
        .map(|s| {
            tracing::debug!("Shortcut triggered: {} - {}", s.module, s.description);
            s.callback
        })
}

/// List all registered shortcuts as a formatted string.
pub fn list_shortcuts() -> String {
    let registry = SHORTCUT_REGISTRY.lock().unwrap();
    if registry.is_empty() {
        return "No shortcuts registered.".to_string();
    }

    let mut result = String::from("Registered shortcuts:\n");
    for s in registry.iter() {
        let modifiers: Vec<&str> = vec![
            if s.ctrl { Some("Ctrl") } else { None },
            if s.shift { Some("Shift") } else { None },
            if s.alt { Some("Alt") } else { None },
        ]
        .into_iter()
        .flatten()
        .collect();

        let mod_str = if modifiers.is_empty() {
            String::new()
        } else {
            format!("{}+", modifiers.join("+"))
        };

        let key_name = format!("{:#x}", s.key_code);
        result.push_str(&format!(
            "  ({}{}{}) {} - {}\n",
            mod_str,
            key_name,
            if s.ctrl || s.shift || s.alt { ")" } else { "" },
            s.module,
            s.description
        ));
    }
    result
}

/// Macro for registering keyboard shortcuts with ergonomic syntax.
///
/// # Example
///
/// ```rust
/// shortcut!(
///     "ztgamemgr",
///     "Add $10,000 to budget",
///     Key::<{ VK_F6.0 as i32 }>::new() + Ctrl,
///     false,  // override
///     || {
///         tracing::info!("Cash added!");
///     }
/// );
/// ```
#[macro_export]
macro_rules! shortcut {
    ($module:expr, $description:expr, $shortcut:expr, $override:expr, || $body:block) => {
        $crate::shortcuts::register_shortcut($module, $description, $shortcut, $override, || $body)
            .unwrap_or_else(|e| tracing::error!("Failed to register shortcut: {}", e))
    };
}

use openzt_detour::generated::ztapp::HANDLE_KEY_DOWN;
use openzt_detour_macro::detour_mod;
use tracing::info;

#[detour_mod]
pub mod zoo_shortcuts {
    use super::*;

    #[detour(HANDLE_KEY_DOWN)]
    unsafe extern "cdecl" fn handle_key_down(param_1: i32) -> u32 {
        if let Some(callback) = super::check_shortcuts(param_1) {
            callback();
            return 0; // Don't call original function
        }
        unsafe { HANDLE_KEY_DOWN_DETOUR.call(param_1) }
    }
}

pub fn init() {
    info!("Initializing keyboard shortcut system");
    if let Err(e) = unsafe { zoo_shortcuts::init_detours() } {
        tracing::error!("Error initializing shortcut detours: {}", e);
    }

    // Example shortcut: Ctrl+Shift+Alt+R prints a test message
    shortcut!(
        "shortcuts",
        "Example: Print test message",
        Ctrl + Shift + Alt + R, // Add Modifiers and a key to create a shortcut
        false,                  // override
        || {
            tracing::info!("Ctrl+Shift+Alt+R shortcut triggered! This is an example shortcut.");
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_key() {
        let k = Key::<0x41>::new();
        let s: Shortcut<true, false, false, 0x41> = k + Ctrl;
        assert!(s.matches(true, false, false, 0x41));
    }

    #[test]
    fn test_key_plus_ctrl() {
        let k = Key::<0x41>::new();
        let s = k + Ctrl;
        assert!(s.matches(true, false, false, 0x41));
        assert!(!s.matches(false, false, false, 0x41));
    }

    #[test]
    fn test_key_plus_alt() {
        let k = Key::<0x41>::new();
        let s = k + Alt;
        assert!(s.matches(false, true, false, 0x41));
    }

    #[test]
    fn test_key_plus_shift_provides_partial_shortcut() {
        let k = Key::<0x41>::new();
        // Key + Shift now produces PartialShortcut, not Shortcut
        let p: PartialShortcut<false, false, true, 0x41> = k + Shift;
        // PartialShortcut cannot be registered, but adding Ctrl makes it valid
        let s = p + Ctrl;
        assert!(s.matches(true, false, true, 0x41));
    }

    #[test]
    fn test_shift_plus_key_provides_partial_shortcut() {
        // Shift + Key also produces PartialShortcut
        let p: PartialShortcut<false, false, true, 0x41> = Shift + Key::<0x41>::new();
        // Adding Alt makes it valid
        let s = p + Alt;
        assert!(s.matches(false, true, true, 0x41));
    }

    #[test]
    fn test_key_plus_ctrl_plus_shift() {
        let k = Key::<0x41>::new();
        let s = k + Ctrl + Shift;
        assert!(s.matches(true, false, true, 0x41));
    }

    #[test]
    fn test_key_plus_all_modifiers() {
        let k = Key::<0x41>::new();
        let s = k + Ctrl + Alt + Shift;
        assert!(s.matches(true, true, true, 0x41));
    }

    #[test]
    fn test_shortcut_accessors() {
        let k = Key::<0x41>::new();
        let s = k + Ctrl + Shift;
        assert_eq!(s.key_code(), 0x41);
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(s.shift());
    }

    #[test]
    fn test_vkkey_code() {
        let s = A + Ctrl;
        assert_eq!(s.key_code(), VK_A.0 as i32);
        assert!(s.ctrl());
    }

    #[test]
    fn test_registerable_trait() {
        let s: Shortcut<true, false, false, 0x41> = Key::<0x41>::new() + Ctrl;
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(!s.shift());
        assert_eq!(s.key_code(), 0x41);
    }

    // ------------------------------------------------------------------------
    // Tests for new ergonomic syntax with const key items
    // ------------------------------------------------------------------------

    #[test]
    fn test_const_key_item_with_modifier() {
        // Test R + Ctrl (key-first order)
        let s = R + Ctrl;
        assert!(s.matches(true, false, false, VK_R.0 as i32));
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(!s.shift());
        assert_eq!(s.key_code(), VK_R.0 as i32);
    }

    #[test]
    fn test_modifier_first_syntax() {
        // Test Ctrl + R (modifier-first order)
        let s = Ctrl + R;
        assert!(s.matches(true, false, false, VK_R.0 as i32));
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(!s.shift());
    }

    #[test]
    fn test_multiple_modifiers_key_first() {
        // Test R + Ctrl + Shift (key-first with multiple modifiers)
        let s = R + Ctrl + Shift;
        assert!(s.matches(true, false, true, VK_R.0 as i32));
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(s.shift());
    }

    #[test]
    fn test_multiple_modifiers_modifier_first() {
        // Test Ctrl + Shift + R (modifier-first with multiple modifiers)
        let s = Ctrl + Shift + R;
        assert!(s.matches(true, false, true, VK_R.0 as i32));
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(s.shift());
    }

    #[test]
    fn test_shift_ctrl_alternative_order() {
        // Test Shift + Ctrl + R (different modifier order)
        let s = Shift + Ctrl + R;
        assert!(s.matches(true, false, true, VK_R.0 as i32));
        assert!(s.ctrl());
        assert!(!s.alt());
        assert!(s.shift());
    }

    #[test]
    fn test_all_three_modifiers() {
        // Test Ctrl + Shift + Alt + R (modifier-first with all three)
        let s = Ctrl + Shift + Alt + R;
        assert!(s.matches(true, true, true, VK_R.0 as i32));
        assert!(s.ctrl());
        assert!(s.alt());
        assert!(s.shift());
    }

    #[test]
    fn test_function_key_shortcuts() {
        // Test function keys with modifiers
        let s1 = F1 + Ctrl;
        assert!(s1.matches(true, false, false, VK_F1.0 as i32));

        let s2 = Ctrl + F12;
        assert!(s2.matches(true, false, false, VK_F12.0 as i32));
    }

    #[test]
    fn test_number_key_shortcuts() {
        // Test number keys with Ctrl (Shift alone produces PartialShortcut)
        let s = NUM5 + Ctrl;
        assert!(s.matches(true, false, false, VK_5.0 as i32));

        // NUM5 + Shift + Alt produces a valid shortcut
        let p = NUM5 + Shift;
        let s = p + Alt;
        assert!(s.matches(false, true, true, VK_5.0 as i32));
    }

    #[test]
    fn test_special_key_shortcuts() {
        // Test special keys
        let s1 = SPACE + Ctrl;
        assert!(s1.matches(true, false, false, VK_SPACE.0 as i32));

        let s2 = Ctrl + ENTER;
        assert!(s2.matches(true, false, false, VK_RETURN.0 as i32));

        let s3 = ESCAPE + Alt;
        assert!(s3.matches(false, true, false, VK_ESCAPE.0 as i32));
    }

    #[test]
    fn test_arrow_key_shortcuts() {
        // Test arrow keys
        let s = UP + Ctrl;
        assert!(s.matches(true, false, false, VK_UP.0 as i32));

        // LEFT + Shift produces PartialShortcut, need to add a modifier
        let p = LEFT + Shift;
        let s = p + Ctrl;
        assert!(s.matches(true, false, true, VK_LEFT.0 as i32));
    }
}
