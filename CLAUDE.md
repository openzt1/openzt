# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ⚠️ CRITICAL: ALWAYS USE openzt.bat (Windows) or specific cargo commands (Linux)

**On Windows**: NEVER use `cargo` directly - ALWAYS use `./openzt.bat` for ANY cargo command (build, check, clippy, test, run, etc.).

The project uses a specific toolchain and target configuration that is managed by `openzt.bat`. Running `cargo` directly will use the wrong toolchain/target.

**On Linux**: Batch files do not work. Use specific cargo commands directly:
- Build: `cargo build --manifest-path openzt/Cargo.toml --lib --target=i686-pc-windows-gnu`
- Check: `cargo check --manifest-path openzt/Cargo.toml --lib --target=i686-pc-windows-gnu`
- Clippy: `cargo clippy --manifest-path openzt/Cargo.toml --lib --target=i686-pc-windows-gnu`
- Test: `cargo test --manifest-path openzt/Cargo.toml --lib --target=i686-pc-windows-gnu`

**⚠️ CRITICAL: ALWAYS specify `--target=i686-pc-windows-gnu` on Linux** - The `thiscall` and `stdcall` ABIs are Windows-specific. Without the target, cargo will use the Linux host target and fail with ABI errors. These errors are expected when cross-compiling and the code will compile correctly on Windows with the proper target specified.

**Windows Examples:**
- ❌ `cargo check` → ✅ `./openzt.bat check`
- ❌ `cargo build` → ✅ `./openzt.bat build`
- ❌ `cargo clippy` → ✅ `./openzt.bat clippy`
- ❌ `cargo test` → ✅ `./openzt.bat test`

If `openzt.bat` is missing a command you need, ADD IT to `openzt.bat` rather than running cargo directly.

## Project Overview

OpenZT is a DLL injection framework for Zoo Tycoon (2001) written in Rust. It provides mod support, bug fixes, and feature enhancements through function detouring and memory manipulation.

**Target**: 32-bit Windows (`i686-pc-windows-msvc`)
**Output**: `openzt.dll` (copied to Zoo Tycoon directory and loaded automatically)

## Critical Rules

1. **NEVER commit Zoo Tycoon assets, code, configs, or decompiled content** - This is a clean-room reimplementation
2. **ALWAYS USE openzt.bat for building, testing, running or just anything that would usually require use of `cargo`** - If openzt.bat is missing functionality add it rather than running cargo directly
3. **New features start behind `experimental` feature flag** in Cargo.toml
4. **All structs must use `#[repr(C)]`** for memory layout compatibility

## Development Commands

**IMPORTANT**: Always use `./openzt.bat` for cargo actions on the openzt crate (build, check, clippy, docs). This ensures correct toolchain selection and target configuration.

### Build Commands
```bash
# Build only (no game launch)
./openzt.bat build                           # Debug with command-console
./openzt.bat build --release                 # Release with command-console
./openzt.bat build --test                    # Debug test build
./openzt.bat build --test --release          # Release test build

# Build and run
./openzt.bat run                             # Debug with command-console
./openzt.bat run --release                   # Release with command-console

# Build and run with --wait flag (waits for game to exit before returning)
./openzt.bat run --wait                      # Debug, wait for exit
./openzt.bat run --release --wait            # Release, wait for exit

# Integration tests
./openzt.bat integration-tests               # Run all integration tests (builds release, displays results)

# Crash capture (builds test DLL, launches game non-interactively under cdb, dumps register/stack state on crash)
./openzt.bat crash-capture                   # Writes to crash_capture_output.txt
./openzt.bat crash-capture --out <file>      # Writes to a custom file

# Debug play (builds the real DLL, launches game under cdb WITH symbols loaded, play manually - a
# symbolized register/stack dump is written automatically on the first crash; see "Getting Real Symbols
# From a Live Crash" below for a bug that needs manual interaction to reproduce, not crash-capture's own
# non-interactive test-DLL battery)
./openzt.bat debug-play                      # Debug build, writes to debug_play_output.txt
./openzt.bat debug-play --release            # Release build
./openzt.bat debug-play --out <file>         # Writes to a custom file

# Code quality checks
./openzt.bat check                           # Run cargo check on openzt
./openzt.bat clippy                          # Run cargo clippy on openzt
./openzt.bat test                            # Run cargo test on openzt

# Documentation
./openzt.bat docs
```

### Lua Console (Runtime Scripting)

The console executes Lua code directly on the game thread. Connect after OpenZT is running:

```bash
cd openzt-console && cargo run
```

**Example Commands**:
```lua
-- List all available functions
help()

-- Search for specific functions
help("cash")

-- Game management
get_date()                           -- Get current in-game date
add_cash(10000)                      -- Add $10000 to budget
enable_dev_mode(true)                -- Enable developer mode
zoostats()                           -- Display zoo statistics

-- Settings
get_setting("AI", "cKeeperMaxTiredness")
set_setting("AI", "cKeeperMaxTiredness", "100")
list_settings()                      -- List all settings
list_settings("AI")                  -- List AI settings only

-- Entity management
get_selected_entity()                -- Get selected entity details
sel_type()                           -- Get selected entity type config
sel_type("-v")                       -- Verbose entity type info
make_sel(9500)                       -- Make entity type selectable

-- World/Habitat info
list_entities()                      -- List all entities in world
list_exhibits()                      -- List all exhibits/habitats
get_zt_world_mgr()                   -- World manager debug info

-- Expansions
list_expansion()                     -- List loaded expansions
get_current_expansion()              -- Get active expansion
get_members()                        -- List expansion member sets

-- Resources
list_resources()                     -- List BF resource directories
list_openzt_mods()                   -- List OpenZT mod IDs
get_string(9211)                     -- Get game string by ID

-- UI
ui("click_continue")                 -- Click continue button
continue()                           -- Shorthand for above
get_buy_tab()                        -- Get current buy tab
```

**Error Handling**:
```lua
-- Functions return (nil, error_string) on failure
result, err = get_string(999999)
if err then
    print("Error: " .. err)
else
    print("Result: " .. result)
end

-- Or check for nil
local date = get_date()
if date then
    print("Date: " .. date)
end
```

**Migration Note**: The old command-style syntax (e.g., `add_cash 1000`) is deprecated. Use Lua function calls (e.g., `add_cash(1000)`) instead. See `MIGRATION_TEMPLATE.md` for details on migrating remaining commands.

### Creating Console Commands

Adding new console commands to OpenZT is done using the `lua_fn!` macro in `openzt/src/scripting.rs`. This macro handles function registration, metadata for `help()`, and Lua integration automatically.

#### Registration Location

All commands are registered in the `init()` function in `openzt/src/scripting.rs` (around line 136). Add your command at the end of this function, before the closing brace.

#### Command Patterns

**No Arguments:**
```rust
lua_fn!("ping", "Test console connectivity", "ping()", || {
    Ok("pong".to_string())
});
```

**Single Argument:**
```rust
lua_fn!("get_string", "Get game string by ID", "get_string(id)", |id: u32| {
    Ok(format!("String: {}", id))
});
```

**Multiple Arguments:**
```rust
lua_fn!("set_setting", "Set a configuration value", "set_setting(section, key, value)",
    |section: String, key: String, value: String| {
        Ok(format!("Set {}.{} = {}", section, key, value))
    }
);
```

**Optional Arguments:**
```rust
lua_fn!("help", "List functions or search by keyword", "help([search_term])",
    |search: Option<String>| {
        match search {
            Some(term) => Ok(format!("Searching for: {}", term)),
            None => Ok("All functions:".to_string()),
        }
    }
);
```

#### Return Value Patterns

**Simple string:**
```rust
Ok("result".to_string())
```

**Tuple (value, error):**
```rust
Ok(("Success message".to_string(), None::<String>))
// On error: Ok((String::new(), "Error message".to_string()))
```

**Unit/void:**
```rust
Ok(())
```

#### Error Handling

Functions should return errors as a tuple with the error string in the second position:

```rust
lua_fn!("get_entity", "Get entity by ID", "get_entity(id)", |id: u32| {
    match find_entity(id) {
        Some(entity) => Ok((entity.name, None::<String>)),
        None => Ok((String::new(), format!("Entity {} not found", id))),
    }
});
```

In Lua, callers check for errors like:
```lua
result, err = get_entity(123)
if err then
    print("Error: " .. err)
else
    print("Result: " .. result)
end
```

## Architecture Patterns

### Module Structure
- **Entry point**: `lib.rs` calls `init()` functions behind feature flags
- **Module pattern**: Each feature module has an `init()` function called from `lib.rs`
- **Feature flags**: Defined in `Cargo.toml` - new features use `experimental` flag

### Memory Management
```rust
// Global state pattern
use once_cell::sync::Lazy;
static GLOBAL_STATE: Lazy<Mutex<MyState>> = Lazy::new(|| Mutex::new(MyState::default()));

// Struct definitions
#[repr(C)]
#[derive(Debug)]
struct GameStruct {
    field: u32,
}
```

### Function Detouring
```rust
// Detour setup (subtract 0x400000 from Ghidra addresses)
static_detour! {
    static MY_DETOUR: unsafe extern "stdcall" fn(u32) -> u32;
}

// Calling game functions
let game_fn: unsafe extern "stdcall" fn(u32) -> u32 = 
    std::mem::transmute(0x12345678); // Full address
```

### Resource Handling
```rust
// Register resource handlers in init()
resource_manager::add_handler("bfb", Box::new(BfbHandler));
```

## Workspace Structure

- **`openzt/`**: Main DLL crate with game hooks and features
- **`openzt-console/`**: Socket-based runtime console
- **`openzt-configparser/`**: Custom INI parser for Zoo Tycoon configs
- **`field_accessor_as_string*/`**: Derive macro crates

## Key Features

### Core Systems
- **Resource Management**: Custom file loading/modification via `resource_mgr/`
- **String Registry**: Game text injection via `string_registry.rs`
- **Lua Scripting**: Runtime Lua execution on game thread via TCP console (port 8080)
- **Settings**: Enhanced INI configuration loading
- **Expansion Packs**: Custom expansion support

### Development Features
- **Feature flags**: `default = ["experimental", "ini"]`, `release = []`
- **Conditional compilation**: Most features behind flags for testing
- **Hot-swappable**: DLL can be reloaded during development

## Testing

### Integration Tests

OpenZT includes an integration testing framework that runs tests in a live game environment. These tests verify mod loading, patch application, and resource management using the actual game engine.

**Running Integration Tests**:
```bash
# Run all integration tests (builds release, launches game, displays results automatically)
./openzt.bat integration-tests
```

The `integration-tests` command:
- Builds the DLL in release mode with the `integration-tests` feature flag
- Launches Zoo Tycoon and waits for tests to complete
- Displays test results automatically after the game exits
- Shows paths to log files for detailed debugging

**Checking Test Results**:
```bash
# View the integration test log
cat "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\openzt_integration_tests.log"

# View detailed OpenZT logs (patch application, errors, etc.)
cat "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\openzt.log"
```

**Test Output**:
```
=== OpenZT Integration Tests ===

Running dependency resolution tests...
  ✓ test_simple_dependency_chain
  ✓ test_circular_dependency_handling
  ✓ test_optional_dependency_warning
  ... (11 tests)

Running patch rollback tests...
  ✓ test_continue_mode_applies_directly
  ✓ test_abort_mode_rolls_back_on_failure
  ... (9 tests)

Running loading order tests...
  ✓ test_category_ordering
  ✓ test_cross_file_habitat_reference
  ... (8 tests)

Running legacy attributes tests...
  ✓ test_legacy_animal_attributes_loaded
  ✓ test_legacy_fence_attributes_loaded
  ... (24 tests)

Results: 52 passed, 0 failed
ALL TESTS PASSED
```

**Test Categories**:

1. **Dependency Resolution Tests** (`openzt/src/integration_tests/dependency_resolution.rs`)
   - Test simple dependency chains
   - Test circular dependency detection and handling
   - Test optional dependencies and warnings
   - Test `before` dependencies
   - Test disabled mods exclusion
   - Test validation of dependency violations

2. **Patch Rollback Tests** (`openzt/src/integration_tests/patch_rollback.rs`)
   - Test patch error handling modes (continue, abort, abort_mod)
   - Verify shadow resource system for transactional patch application
   - Test patch operations (set_key, merge, delete, etc.)

3. **Loading Order Tests** (`openzt/src/integration_tests/loading_order.rs`)
   - Verify deterministic mod definition file loading order
   - Test category ordering (NoPatch → Mixed → PatchOnly)
   - Verify alphabetical sorting within categories
   - Test cross-file habitat/location references in patches

4. **Legacy Attributes Tests** (`openzt/src/integration_tests/legacy_attributes.rs`)
   - Test loading of legacy entity attributes from .cfg files
   - Test default subtype assignment (animal, staff, fence, wall)
   - Test explicit subtype specification
   - Test patch-based legacy attribute substitution
   - Test fallback behavior for invalid subtypes
   - Test cNameID string ID resolution

**Creating New Tests**:

1. Add test functions to appropriate test module:
```rust
pub fn run_all_tests() -> Vec<TestResult> {
    vec![
        test_existing_feature(),
        test_your_new_feature(),  // Add here
    ]
}

fn test_your_new_feature() -> TestResult {
    let test_name = "test_your_new_feature";

    // Setup test data
    // ...

    // Perform test operations
    // ...

    // Verify results
    if expected == actual {
        TestResult::pass(test_name)
    } else {
        TestResult::fail(test_name, format!("Expected {}, got {}", expected, actual))
    }
}
```

2. For tests requiring mod resources, use the embedded test mod pattern:
```rust
// In loading_order.rs - embed test TOML files
const DEF_FILE: &str = include_str!("../../resources/test/your-test/defs/test.toml");

// Add to create_test_mod_file_map()
file_map.insert(
    "defs/test.toml".to_string(),
    DEF_FILE.as_bytes().to_vec().into_boxed_slice(),
);
```

3. Create test resource files in `openzt/resources/test/your-test/`:
```
your-test/
├── meta.toml
└── defs/
    └── test.toml
```

**Embedded Test Mod Pattern**:

Integration tests use an embedded mod approach where test resources are compiled directly into the binary:

- Test files are embedded using `include_str!()` and `include_bytes!()`
- No ZIP file creation or installation required
- Changes to test resources take effect on next build
- Zero runtime overhead - resources are in memory at compile time

**Important Notes**:

- Tests run in a live game environment with initialized memory structures
- The game launches and exits automatically when tests complete
- Use the `--wait` flag to wait for the game to exit before returning control (recommended for automated workflows and CI)
- Test log is always written to `C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\openzt_integration_tests.log`
- Load order tracking is only enabled with `integration-tests` feature flag
- Tests create temporary files (e.g., `animals/test.ai`) for verification
- **Habitat/Location Registration**: Always use the TOML key identifier (e.g., "test_habitat_a"), NOT the display name (e.g., "Test Habitat A") when looking up habitats/locations in tests

### Live Reimplementation-Comparison Tests

Separate from integration tests: the `reimplementation-tests` feature (`openzt/src/reimplementation_tests/`, always enabled by `openzt-test-dll`) builds standalone instances of a reimplemented struct, calls the real vanilla function via `.original()()` on one and the Rust reimplementation on the other, and compares results/state field-by-field. Same `./openzt.bat build --test` / `run --test --wait` workflow as above; results append to `openzt_test.log` (`OPENZT_TEST_LOG` env var overrides the path).

**⚠️ Never free real vanilla output through Rust's allocator, or vice versa.** If a class manages its own heap objects (e.g. `ZTThoughtMgr`'s intrusive linked list, allocated through vanilla's own small-object freelist when reached via `.original()`, vs. `Box` when built by test/reimplementation code), calling `Box::from_raw`/`drop` on a node that real vanilla code allocated - or letting vanilla code's own free path touch a `Box`-allocated node - is a genuine cross-allocator heap corruption bug, not just a leak. It will crash, but Windows' Fault Tolerant Heap can silently absorb the crash (no dialog, no WER crash dump, `openzt.log` empty) after a few occurrences, making it look like the game "just exited" - a reboot is sometimes needed to see a real crash again after FTH kicks in. When a real, undetoured mutator is called live in a test and might allocate/link nodes of its own, build a leak-only teardown path for that side (free only what your own code allocated - the sentinel/outer struct - and deliberately leak anything vanilla's own allocator produced) rather than reusing the normal Box-walking cleanup. See `ztthoughtmgr.rs`'s `live_support::destroy_standalone_mgr_leaking_nodes` for a worked example.

**⚠️ A `generated.rs` destructor-shaped entry may not take the class's own pointer.** More than once (`ztgamemgr_menumusichandler`'s `MENU_MUSIC_HANDLER_0`; `ztshowstate`'s original `ZTSHOW_STATE`, since corrected) a destructor address the generator pass clustered into one class's module turned out to actually be a *different* class's own destructor (typically the derived class that embeds the named one), which tears the named class down only as an inlined tail - after first adjusting `this` by some base-class offset and restoring *its own* vtable. Calling such an entry directly with the named class's bare pointer (e.g. to build a standalone live-test fixture) reads/writes past that allocation and can crash the live battery outright, with no exception logged (see the Fault Tolerant Heap note above - this looks exactly like that). Before calling any destructor-shaped `generated.rs` entry against a standalone fixture, read its `.asm`'s first few instructions: an unexpected `ADD ECX, N`/similar `this`-adjustment, or a vtable restore for a class other than the one you're testing, means it belongs to that other (usually enclosing) class and needs a `this` pointer embedded inside a real instance of *it*, not a bare pointer to the class under test. If you find one, this is exactly the kind of generator-pass mislabeling the section above says to surface rather than hand-edit.

**⚠️ A module's live-test detours must be wired into `reimplementation_tests::init()`, separately from `lib.rs`.** `openzt-test-dll`'s `DllMain` calls `openztlib::reimplementation_tests::init()` directly - it never runs `lib.rs`'s own `LOAD_LANG_DLLS_DETOUR`-gated boot cascade at all. `reimplementation_tests::init()` (top of `openzt/src/reimplementation_tests/mod.rs`) keeps its own separate, explicit list of which modules' `::init()` to call for the live battery. A module wired only into `lib.rs`'s cascade (see "Wire a new module into `lib.rs`" in the Reimplementation Pattern section below) never gets its detours installed under `run --test`/`build --test` - and this fails **silently**: `init_detours()` for the missing module is simply never called, there is no error to see, and every `.hooked()` call in that module's own live tests quietly falls through to real, un-detoured vanilla code. A comparison test can still *pass* this way, by coincidence (real and ported behavior happen to agree on the cases exercised), giving zero actual coverage while looking green. **Always add both**: the module's `::init()` call to `reimplementation_tests::init()`'s list, and a `<CLASS>_DETOURS_ENABLED` test alongside its other live tests (`ztgamemgr_menumusichandler`/`ztsoundscape`/`zoostatus`/`ztshowstate` all follow the same `live_support::detour_status()` → per-detour `.is_enabled()` pattern) - the enabled-check is the only thing in the battery that catches a missing-wiring gap directly, rather than it hiding behind a false-positive comparison test.

#### How to add a new live test

The mechanics below aren't obvious from the file's own size (~12k lines) or from a single example - they're
spread across the `RegisteredTest`/list-builder plumbing near the top of `openzt/src/reimplementation_tests/mod.rs`
and the `io_redirect` submodule. Read an existing test close to what you're adding (e.g. `ZTSHOWMGR_SAVE_LOAD`
for a save/load round trip, `ZTSHOW_GET_SHOW_SCRIPT_STATE` for a pure fixture comparison) before writing a new
one from scratch.

1. **Write the test function**: a private `fn run_<name>_test(failure_log: &mut Option<std::fs::File>) -> bool`
   in the same big `mod` as every other test (so it can see the top-of-file `use crate::{...}` imports).
   The bool's polarity is "did it fail" (`true` = failed), not "did it pass" - easy to get backwards. Standard
   shape: accumulate `let mut failures: Vec<String> = Vec::new();` as you go, then end with
   ```rust
   if failures.is_empty() {
       write_success_line(failure_log, test_name);
       false
   } else {
       for msg in &failures { error!("{}: {}", test_name, msg); }
       if let Some(log_file) = failure_log {
           let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
       }
       true
   }
   ```
2. **Register it**, don't just define it - a test function that's never pushed onto a list silently never runs
   (no error, just absent from the log). Add `tests.push(RegisteredTest { name: "YOUR_NAME", run: run_your_name_test });`
   to exactly one of the three ordered-list builder functions, chosen by what the test needs:
   - `early_tests()` - no live zoo needed, runs first.
   - `always_late_tests()` - no live zoo needed, runs after `early_tests()` but before the live-zoo gate; where
     most standalone-fixture comparison tests belong (matches `ztshowstate`'s own `_DETOURS_ENABLED`/`_INIT`/
     `_SAVE_LOAD_ROUNDTRIP` entries).
   - `live_zoo_tests()` - needs `run_load_live_zoo` to have already succeeded (real `GLOBAL_*` globals
     populated from an actual loaded save); auto-skipped with an explicit `(skipped: live zoo not loaded)` log
     line otherwise, so a gap in the log is never silent.
   These three lists are the single source of truth for the battery's total expected-test count (the
   start/finish markers) - no separate counter to update anywhere.
3. **For a save/load-style test**, use the `io_redirect` submodule to capture/replay bytes without a real file:
   `io_redirect::begin_capture()` / `end_capture() -> Vec<u8>` redirect the vanilla `WRITE_BYTES_TO_FILE` calls
   made inside whatever you call next into an in-memory buffer; `io_redirect::begin_replay(bytes: Vec<u8>)` /
   `end_replay()` do the same for reads (`DEALLOCATE`). Wrap exactly the `.hooked()`/`.original()` call whose
   I/O you want captured - not more, not less - between the matching begin/end pair.
4. **For a standalone-instance comparison test**, reuse the class module's own `#[cfg(feature =
   "reimplementation-tests")] pub(crate) mod live_support` (see "File/module shape for a class reimplementation"
   below) rather than hand-rolling allocation here - `build_standalone_*`/`destroy_standalone_*` helpers already
   exist for most ported classes. Import the module at the top of `reimplementation_tests/mod.rs`'s own `use
   crate::{...}` block as `<module>::{self, live_support as <alias>_live_support}`, matching the existing
   `ztshow_live_support`/`showmgr_live_support`/etc. aliases.
5. If this is the module's first live test, it also needs the two wiring steps from the warnings above: an
   `::init()` call in `reimplementation_tests::init()`, and a `<CLASS>_DETOURS_ENABLED` test.

### Game Launch Checks

The build script automatically checks if Zoo Tycoon is already running before attempting to launch:

```bash
./openzt.bat run --release

# If Zoo Tycoon is already running, you'll see:
# ERROR: Zoo Tycoon is already running.
# Please close the existing instance before launching a new one.
```

This prevents DLL copy failures due to file locks and ensures clean testing environments.

### Manual cdb Debugging

For live investigation beyond `crash-capture`'s single "run to crash, dump, quit" shape (e.g. tracing a
sequence of calls while a human plays normally, or checking whether a particular code path is even
reached) - launch `zoo.exe` under `cdb.exe` directly rather than through `openzt.bat`:

```bash
CDB="C:\Program Files (x86)\Windows Kits\10\Debuggers\x86\cdb.exe"   # x86 build matches zoo.exe's 32-bit target
ZOO='C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\zoo.exe'
"$CDB" -G -c "bp <addr1> \".echo LABEL1;g\";bp <addr2> \".echo LABEL2;g\";g" "$ZOO" > trace.log 2>&1
```

Run this as a genuinely backgrounded process (not `&`/`nohup`/`disown` tacked onto a shell command - that
combination has caused spurious early exits in this environment; let the tool's own background-run
mechanism handle it) so the game stays interactive for a human to actually play while cdb watches
silently in the background, then read the log afterward.

**Gotchas, in order of how often they bite:**

- **Only one instance can be starting/running at a time.** Relaunching (even after a `taskkill`) while a
  previous instance is still up or still tearing down gets rejected near-instantly, with no exception and
  no dialog - this looks exactly like a crash from the log alone, but isn't one. Confirm the previous
  instance is genuinely gone before relaunching (see next point for how), and don't assume Windows'
  **Fault Tolerant Heap** silently absorbing repeated crashes (see the note above) is the explanation by
  default - check with the user, since FTH may be deliberately disabled for `zoo.exe` in their environment.
- **`tasklist` does not reliably see `zoo.exe`/`cdb.exe` in this environment**, even while they are
  genuinely running and visible on the user's desktop (a session boundary, most likely) - it can report
  "no tasks running" for a process the user is actively using. Use `wmic process where "name='zoo.exe'"
  get ProcessId,ExecutablePath` (or equivalent) instead; treat a `tasklist` "not found" result as
  inconclusive, not proof the process exited.
- **Never `bp` a raw address that this crate's own `#[detour(...)]` already hooks.** At runtime that
  address holds a jump into our own Rust code, not the original vanilla bytes; cdb's `int3` patch collides
  with it, and continuing past the breakpoint can land execution in garbage (observed: an "Access
  violation" at `hooked_address+1`, decoding into a nonsense instruction) - a pure debugging artifact, not
  a real bug. Only breakpoint addresses you've confirmed are genuinely un-hooked (check for a matching
  `#[detour(NAME)]`/`generated.rs` entry first), or verify a suspicious crash isn't this artifact by
  reproducing the same scenario with that specific breakpoint removed before trusting it.
- Quote each `bp <addr> "<action>;g"` breakpoint action so cdb treats it as one multi-command string, and
  pass the *entire* breakpoint set as a **single** `-c` argument (`bp ...;bp ...;g`) - cdb does not chain
  multiple `-c` flags into a sequence, it only keeps the last one.

### Getting Real Symbols From a Live Crash

`./openzt.bat run`/`run --release` show a crashing frame as a bare `zoo+0xNNNNN`/`res-openzt.dll+0xNNNNN`
offset with no function name, because nothing tells cdb where our own `.pdb` lives. For a bug that needs a
human to actually play to reproduce (as opposed to `crash-capture`'s own non-interactive "run the test DLL's
battery to completion or crash" shape, which never reaches a real playable state at all - it runs the
`reimplementation-tests` battery and calls `std::process::exit()` immediately after), use:

```bash
./openzt.bat debug-play               # debug build
./openzt.bat debug-play --release     # release build, matches a --release repro
./openzt.bat debug-play --out x.txt   # custom output path (default debug_play_output.txt)
```

This builds the real (non-test) `openzt.dll`, launches `zoo.exe` under cdb with the matching `.pdb`'s
directory on the symbol path, lets the game run fully interactively (play normally - cdb only watches), and
on the first unhandled exception writes a properly symbolized `kv`/`r` dump before quitting. Reproduce the
bug, then read the output file.

**If doing this by hand instead** (e.g. to add custom breakpoints), the pieces `debug-play` automates:

- Pass `-y "srv*;<path-to-target-profile-dir>"` (the directory containing `openzt.pdb`, e.g.
  `target\i686-pc-windows-msvc\release`) - `srv*` alone only fetches Microsoft's own OS symbols, never our
  own. The `.pdb` must be the one produced by the exact build currently deployed as `res-openzt.dll`;
  rebuilding without redeploying (or vice versa) silently desyncs addresses from symbols.
- `res-openzt` (with the hyphen, matching the actual filename) works for the module-*load* event filter
  (`sxe ld:res-openzt`), but cdb sanitizes the module name to `res_openzt` (underscore) for its own
  expression/symbol namespace - an expression like `ln res-openzt+0xNNNNN` fails outright ("Couldn't resolve
  error"; the hyphen is parsed as subtraction) and must be written `ln res_openzt+0xNNNNN`.
- Run `.reload /f res-openzt.dll` right after the module-load stop to force cdb to actually pick up the
  symbol path for it - it doesn't always resolve automatically at load time.
- Prefer letting the crash happen and reading cdb's own `kv`/`r` output over manually probing
  `ln <module>+<offset>` ahead of time. `ln`'s "nearest symbol" search can land somewhere wildly unrelated
  across a large gap with nothing symbolized in between - observed once resolving an address that was
  actually inside `zthabitatmgr`'s own habitat code as "~19KB into a zip/crc32 decompression reader",
  because nothing between the two had a distinct symbol cdb could see.
- A **small, non-`pub` function has no distinct linkable symbol of its own** - this bites hardest inside a
  `#[detour_mod] mod { ... }` block, where every detour trampoline (`#[detour(NAME)] unsafe extern
  "<abi>" fn ...`) is private. A `kv` frame landing inside one shows up as the nearest `pub` symbol in that
  same module instead (e.g. every private trampoline in `hooks_zthabitatmgr` prints as
  `hooks_zthabitatmgr::init_detours+0xNNNN`) - don't read that as "the crash is inside `init_detours`" just
  because that's the name in the trace. The actual crashing frame (if it's a `pub fn`, e.g. a class's own
  ported method) usually still resolves correctly; it's specifically the *callers* one or two frames up,
  landing inside another detour's own trampoline, that get this misattribution.
- Only one debugger can attach to a process at a time. Attaching a second `cdb -p <pid>` session to a
  process already running under a first cdb instance fails immediately (`Cannot debug pid ..., Win32 error
  87`) - stop the first session (`Stop-Process` on the `zoo.exe` pid, not just closing the cdb window) before
  attaching a second.
- The Windows Event Log's own Application Error record for the crash (`Get-WinEvent -FilterHashtable
  @{LogName='Application'; ProviderName='Application Error'}`) reports a bare "Fault offset" that's already
  a plain module RVA, with no debugger needed at all - a fast first triage, and a good sanity check that a
  live cdb capture's own `eip - <module ModLoad base>` lines up with what Windows independently recorded.

### Manual Testing

For features not covered by integration tests:

1. Build and run with `./openzt.bat run --release`
2. Verify features work in-game
3. Test console commands if applicable
4. Check for game crashes or memory issues

## Reimplementation Pattern

This section documents how a vanilla `ZT*Mgr`/`BF*Mgr` class gets fully reimplemented in Rust (see
`openzt/plans/zt-mgr-classes-reimplementation-roadmap.md` for which classes are done/candidates). Established
by `ztmarketing.rs`/`ztresearch.rs`/`ztthoughtmgr.rs`/`ztmegatilemgr.rs`.

### `openzt-detour/src/generated.rs`

- Auto-generated from a Ghidra analysis pass run **outside this repo** - there is no generator script checked
  in. **Never hand-edit this file** - not existing entries (e.g. a wrong signature/ABI), not entries the pass
  missed, not even comment-only annotations: a regeneration silently discards every hand-edit, and an entry
  that regenerates back to a wrong signature builds detours that corrupt the stack in every un-ported caller.
  When the pass's output is wrong or incomplete, **surface it to the user instead** - the address, the
  `.asm`/caller evidence, and what the entry should say - so the fix can go into the generator itself, and
  wait for the next regeneration to correct the file.
- One `pub mod <lowercase_classname> { ... }` block per C++ class (free functions live under `standalone` or a
  UI-area module like `ztui`). Each function is `pub const <SCREAMING_NAME>: FunctionDef<unsafe extern
  "<abi>" fn(...) -> R> = FunctionDef{address: 0x..., function_type: PhantomData};`.
- `address` is always the **raw Ghidra virtual address, untouched** - Zoo Tycoon's `.exe` has no ASLR and
  always loads at its preferred base `0x00400000`, so a Ghidra VA already equals the runtime VA for *code*.
  This is different from **data** addresses (globals/statics), which get resolved at runtime as
  `get_module_base("zoo.exe") + RVA` (RVA = Ghidra address minus `0x400000`) - see `globals.rs`'s
  `CachedGlobalInstance` entries for the pattern. Don't confuse the two: function-table entries in
  `generated.rs` need no base-address math; ad-hoc global/struct-field addresses computed in `openzt/src` do.
- Every entry carries a `#[cfg_attr(feature = "detour-validation", validate_detour("class/method"))]`
  attribute. This is currently inert scaffolding - no `detour-validation` feature or `validate_detour` macro
  exists in the repo - just copy the attribute verbatim on any new entry for consistency, don't try to wire it
  up.
- `FunctionDef::original()` returns the real vanilla function - in debug builds it routes through the
  trampoline registered by `FunctionDef::detour()` (a process-global hook registry in
  `openzt-detour/src/lib.rs`, `#[cfg(debug_assertions)]`), so calling `.original()` on a hooked address still
  reaches vanilla, never our own detour; in release builds it is a raw `retour::Function::from_ptr` cast of the
  stored address (zero cost, no registry), so there calling `.original()` on a hooked address re-enters the
  detour. The battery's `MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE` test pins the debug routing.
  `FunctionDef::hooked()` is the deliberate re-entry API: it is the raw address cast in **every** build (what a
  vanilla caller executing the address runs - our detour if one is installed), for the few call sites whose
  documented intent is to invoke the hook itself (e.g. `ztshowui.rs`'s `load_display_string` wanting
  string-registry-aware `BFApp::loadString`). Calling `.original()` on a function you have *not* detoured is
  always safe. Calling it *from inside that same function's own detour* is not safe in release (see below) -
  use `<NAME>_DETOUR.call(...)` there regardless of profile.

### Consuming a vanilla return value as a bool - check for undefined upper bytes

Before comparing any `.original()(...)`/`.hooked()(...)` return value against `0` or otherwise treating it
as a bool, open that function's own decompile and look at its literal `return` statements. Ghidra frequently
decompiles a function that really only sets `AL` as `return CONCAT31((int3)(garbage >> 8), local_flag)` -
explicitly packing an undefined/garbage upper 3 bytes around the one real byte - or the sibling shape
`some_reg & 0xffffff00` (forcing the low byte to a fixed value while leaving the upper bytes as whatever was
already in the register). Real vanilla callers always match this with `TEST AL, AL` (low byte only, never
the full register). A full-width comparison against `0` is a genuine bug that happens to work whenever the
garbage bytes are zero (e.g. an isolated call with a clean stack) and silently breaks once they aren't (e.g.
a dense burst of repeated calls each leaving different register garbage behind) - so a narrow test passing
is not strong evidence this class of bug is absent. Found twice so far in this codebase purely via live
crashes/corruption (`ZooStatus::fChance`, and `ZTHabitatMgr::createHabitat`'s `doTankCheck`/
`ZTTankExhibit::removeIllegalEntities` reads) - check for it proactively during any signature audit rather
than waiting for a report. Use `util.rs`'s `low_byte_bool(value: u32) -> bool` at the call site instead of a
raw `!= 0`/`== 0` comparison whenever a decompile shows this shape.

**When you have live Ghidra access, prefer fixing the root cause over masking at every call site.** If the
function's real C++ return type is genuinely `bool` (a membership/comparison test used as a yes/no gate at
every caller, not a wider value callers happen to truncate), retype its return in Ghidra directly
(`variables` → `set_prototype`, e.g. `bool isAmphibiousNeighbor(void * this, uint param_1)`) rather than
leaving it `uint`/`undefined4` and masking with `low_byte_bool` on the Rust side. Ghidra then emits clean
`return a != b;`-shaped code instead of the `CONCAT31` packing, for every caller at once. Reach for
`low_byte_bool` only when you're not sure the real return type is `bool`, or when it's confirmed wider and
callers are just reinterpreting it.

### Detouring a function (`#[detour_mod]` / `#[detour(NAME)]`)

Provided by `openzt-detour-macro`. Shape (see `ztthoughtmgr.rs`'s `thought_save_detours` module or
`ztmegatilemgr.rs`'s `megatilemgr_detours` module for full worked examples):

```rust
use openzt_detour::generated::<classname>::{SAVE, LOAD};

#[detour_mod]
mod detours {
    use super::*;
    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        unsafe { ref_from_memory::<MyMgr>(this) }.save(file)
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise <classname> detours: {e:?}");
    }
}
```

- `#[detour_mod]` generates one `static <NAME>_DETOUR: LazyLock<GenericDetour<...>> = ...` per `#[detour(NAME)]`
  function in the block, plus an `init_detours()` that `.enable()`s each. The detour function's own
  `extern "<abi>"` annotation is read directly by the macro and must match the `FunctionDef`'s ABI/signature
  exactly - there's no separate thiscall-specific wrapper, just Rust's native `extern "thiscall"` support with
  `this: *const u32` as the first parameter.
- Use `NAME.original()(...)` to call a vanilla function's real body when you have **not** hooked that same
  function (e.g. calling a different helper, or calling through from one class's detour into another
  class's un-hooked method). In debug builds it stays correct even for addresses that *are* hooked (it routes
  through the hook registry's trampoline); in release it is a raw address cast, so there it must stay off the
  hooked addresses (see the `generated.rs` section above and `FunctionDef::hooked()` for deliberate re-entry).
- Use the macro-generated `<NAME>_DETOUR.call(...)` whenever calling the real body **of the function your
  current code is itself a detour for** (its address has been patched to jump into your detour, so in release
  `.original()` there would recurse into yourself). This is the only correct call-through in every profile -
  live-test "real vanilla" poles built on it (e.g. `ztgamemgr_menumusichandler::live_support::real_*`,
  ztawardmgr's `call_real`) stay release-safe because of it. See `resource_manager/hooks.rs`'s `CONSTRUCTOR`
  detour for the pattern (run `CONSTRUCTOR_DETOUR.call(this_ptr)` first, then layer additional Rust logic on top).
- A **partial-override** detour (replace behavior for one input/condition, delegate to the real function for
  everything else) uses the same `<NAME>_DETOUR.call(...)` mechanism inside a `match`/`if` - see
  `resource_manager/hooks.rs`'s `zoo_ui_general_get_info_image_name` for the shape. There's no dedicated
  "partial override" macro - it's a plain conditional around the call-through.

### File/module shape for a class reimplementation

One file per class in `openzt/src/` (e.g. `ztthoughtmgr.rs`): module doc comment explaining the vanilla class
and any allocator/memory-safety caveats -> `#[repr(C)]` struct(s) mirroring vanilla layout with a
`size_of` assertion, **only if the reimplementation needs to read/write vanilla's own memory in place**
(see "Two reimplementation styles" below) -> `impl` blocks with the real logic as plain Rust methods, kept
separate from the detour glue -> one or more `#[detour_mod] mod ... { }` blocks (split into
purpose-grouped submodules for a large class, e.g. `ztthoughtmgr.rs`'s `thought_accessor_detours`/
`thought_mutator_detours`/`thought_save_detours`/`thought_dtor_detour`) -> a top-level `pub fn init()`
aggregating each submodule's `init()` -> `#[cfg(feature = "reimplementation-tests")] pub(crate) mod
live_support { ... }` with test-only helpers -> `#[cfg(test)] mod tests { ... }` for plain logic unit tests.

Wire a new module into `lib.rs`: add `mod <name>;` near the other `mod zt*mgr;` declarations, and
`<name>::init();` inside the `if cfg!(feature = "experimental") { ... }` block. **If the module has live
tests, it also needs a separate `<name>::init();` call in `reimplementation_tests::init()`** - see the
Testing section's "Live Reimplementation-Comparison Tests" warning above on why this is a second,
independent wiring step, not implied by the `lib.rs` one.

### Two reimplementation styles - pick based on whether other vanilla code reads the class's raw memory

1. **Vanilla-layout-compatible** (`ZTThoughtMgr`, `ZTMegatileMgr`): the global is a pointer to a
   heap-allocated instance; a `#[repr(C)]` struct mirrors vanilla's fields exactly, and Rust methods read/write
   that memory in place (sometimes alongside a side `HashMap` keyed by pointer for data that doesn't fit
   vanilla's layout). Necessary when other, un-decompiled/un-detoured vanilla code might still read the
   struct's raw fields directly (can't fully rule this out), or when the global is heap-allocated via a
   constructor that can also be run against a fresh allocation for standalone side-by-side testing.
2. **Fully independent Rust store** (no vanilla-layout struct at all): only viable when every vanilla code
   path that reads/writes the class's fields has been enumerated (grep the whole decompile corpus for the
   class name *and* the raw field addresses directly, not just method names - a caller can read a field
   inline without going through any method call) and every one of them is being detoured/reimplemented too.
   Vanilla's own copy of the data is then left completely alone (never read or written by Rust) and becomes
   inert dead weight. This sidesteps the cross-allocator hazard below entirely, at the cost of losing the
   ability to do standalone/side-by-side memory-diff testing if the class's constructor hardcodes a fixed
   global address (can't be run against a second instance) - live tests then have to compare `.original()`
   *behavior/return values* against the Rust store's outputs instead of diffing shared memory.

### Cross-allocator memory safety (style 1 only)

**Never free real vanilla-allocated output through Rust's allocator, or vice versa.** If a class manages its
own heap objects reachable through a still-live, undetoured vanilla code path (e.g. a linked list node
allocated through vanilla's own small-object freelist when reached via `.original()`, vs. a `Box` when built
by test/reimplementation code), calling `Box::from_raw`/`drop` on a vanilla-allocated node - or letting
vanilla's own free path touch a `Box`-allocated node - is genuine heap corruption, not just a leak. It can
crash, but Windows' Fault Tolerant Heap may silently absorb a few occurrences (no dialog, empty `openzt.log`,
looks like the game "just exited") - a reboot is sometimes needed to see a real crash again once FTH kicks in.
Build a leak-only teardown path for the side that might hold vanilla-allocated nodes (free only what your own
code definitely allocated, deliberately leak the rest) rather than reusing a normal Box-walking cleanup - see
`ztthoughtmgr.rs`'s `live_support::destroy_standalone_mgr_leaking_nodes` for a worked example.

## Ghidra MCP (live identification work)

When a Ghidra MCP server is connected, prefer it over manual decompile copy-paste for identification work
(naming unnamed/`FUN_`/`cls_`/`meth_` helpers, resolving ICF-folded shared-label functions, fixing missed
parameters). Gotchas hit in practice:

- Most calls (`get_code`, `variables`, ...) are async - they return a `task_id`; poll with `get_task_status`
  until it completes, don't assume the first response is the result.
- `rename_symbol`/`batch_rename`: pass the **bare current function name** as `identifier` (e.g.
  `meth_0x412fb8`) - not the namespace-qualified name (`BFTile::meth_0x412fb8`) and not the address, both of
  which fail with "Function not found". `new_name` does accept a fully qualified name
  (`msvc_std::list<uint>::erase`).
- `variables` with `action: set_prototype`: pass `function_address` (address works here), and **omit the
  calling-convention keyword** (`__thiscall` etc.) from the `prototype` string - including it fails with
  "Can't resolve return type". Ghidra keeps the function's existing calling convention automatically.
- Before applying a proposed name, `search_functions_by_name` for it first. A name can already exist at a
  *different* address - proof the helper is one of this codebase's known ICF-folded/shared-label functions
  with multiple real instantiations (same phenomenon as `cls_0x4012a6`/`meth_0x40a01d`). Renaming to a
  colliding name fails outright rather than overwriting, so disambiguate (e.g. an address suffix) instead of
  guessing blind.
- Still applies even with live access: never hand-edit `generated.rs` directly (see above) - use Ghidra MCP to
  confirm/apply names and fix signatures in the live project, then regenerate as usual.
- To pin down an unknown container element size precisely (rather than guessing from context), find which
  size-classed `PoolAlloc` freelist bucket (`DAT_006380XX`) its constructor/destructor pushes/pops from. The
  bucket index formula seen throughout this codebase is `idx = (byteSize - 1) >> 3`, with the freelist array
  at `DAT_00638000 + idx*4`; bucket `idx` covers sizes in `(idx*8, idx*8+8]`. Two different addresses that
  both round to the same bucket are not proof they share a size - invert the formula to get the real range.
- When a name collides with one already applied at a different address (`Function with name 'x' already
  exists in namespace 'y'`), that's confirmation the helper is one of this codebase's known ICF-folded/
  shared-label functions with multiple real instantiations - disambiguate (an address suffix, or a
  byte-size tag like `tree24`/`tree36` when the instantiations genuinely differ in size) rather than
  guessing which one is "real."
- `rename_symbol`'s `identifier` matches by the function's **unqualified short name only** - it ignores the
  namespace prefix, and resolves an ambiguous match (more than one function sharing that short name,
  anywhere in the binary) to the **lowest address** among them, silently. Never reuse a short name across
  two of your own renames, even under different namespaces you intend to keep distinct - check
  `search_functions_by_name` on the exact short name for collisions (including pre-existing, unrelated ones)
  before renaming. If a rename lands on the wrong address, fix it with a temporary-rename dance: rename the
  wrongly-grabbed function to a throwaway placeholder, rename the real target (now uniquely resolvable),
  then rename the placeholder back.
- `create_function` at an address can fail with "may not contain valid code or may overlap an existing
  function" even when the surrounding instruction boundaries are genuinely clean (confirmed via
  `get_basic_blocks` on the neighboring functions and gap-filling `disassemble_at` first) - the real cause
  wasn't identified. If it keeps failing after confirming clean boundaries, defining the function manually
  in the Ghidra GUI is a reliable fallback; `get_code`/`get_basic_blocks` work normally on it afterward.

## Cross-checking real names via the macOS decompile

The Windows OOAnalyzer pass invents placeholder names (`meth_0x...`, `cls_0x...`, `FUN_...`) for everything
it can't identify, but Zoo Tycoon's macOS build kept real, unstripped C++ symbol names. When a placeholder
looks like it's a genuine **game-logic method** (a `ZTHabitat`/`ZTAnimal`/`BF*`-style class method, not a
generic STL/allocator internal), check for a macOS decompile before inventing a name:

- Real names live in `private/resources/macos-decompiles/` as `ClassName_methodName.c`. `Grep` there for the
  class name first (e.g. `ZTHabitat_`) - if the specific method already has a file, read it directly.
- If a Ghidra MCP session is connected, the user can also open the macOS binary as a second program
  (`list_binaries` will show it once loaded; pass `program_name` to target it) and query it live the same way
  as the Windows one - useful when the static export in `private/resources/macos-decompiles/` doesn't cover
  the function you need, or looks stale.
- **Match by structure, not just class name.** Confirm a macOS candidate by comparing the actual call
  sequence/branch shape against the Windows decompile (same helper calls in the same order, same early-outs,
  same loop shape) - don't accept a name just because it's the right class and "sounds right." Example from
  this codebase: `ZTHabitat::getCloseOutsideTile`'s macOS body (walk neighbors, skip via `isAmphibiousNeighbor`,
  collect survivors into a temp list, `rand() % size` to pick one, return a stored field) matched the Windows
  `meth_0x448cb7` step-for-step, including the exact predicate call site that turned out to be
  `isAmphibiousNeighbor` too - that's a real confirmation, not a guess.
- **This technique is for game-logic methods, not compiler-generated internals.** STL/allocator helpers
  (`vector<T>::_Insert_n`, `_Tree::insert`, etc.) get their own compiler- and platform-specific internal
  shapes and mangled names on macOS (e.g. `_insert__Q23std42__list_deleter<PCv,...>`) that don't correspond
  1:1 with the MSVC/Windows internals - don't go looking for a macOS name for those; keep using the
  `msvc_std::*` generic-role naming convention instead.

## Code Quality

- Avoid obvious comments that restate code
- Document complex game memory layouts and reverse engineering discoveries
- Use meaningful variable names for game offsets and structures
- Follow existing patterns for detour setup and global state management
