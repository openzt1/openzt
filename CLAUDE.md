# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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

### Game Launch Checks

The build script automatically checks if Zoo Tycoon is already running before attempting to launch:

```bash
./openzt.bat run --release

# If Zoo Tycoon is already running, you'll see:
# ERROR: Zoo Tycoon is already running.
# Please close the existing instance before launching a new one.
```

This prevents DLL copy failures due to file locks and ensures clean testing environments.

### Manual Testing

For features not covered by integration tests:

1. Build and run with `./openzt.bat run --release`
2. Verify features work in-game
3. Test console commands if applicable
4. Check for game crashes or memory issues

## Code Quality

- Avoid obvious comments that restate code
- Document complex game memory layouts and reverse engineering discoveries
- Use meaningful variable names for game offsets and structures
- Follow existing patterns for detour setup and global state management
