# Contributing to OpenZT

We welcome contributions from everyone! This document provides guidance for developing OpenZT.

## Development Environment

### Prerequisites
- Rust stable toolchain 1.85 or later (i686-pc-windows-msvc)
- Zoo Tycoon (2001) installed
- Windows 32-bit development environment

### Quick Setup

```bash
# Clone repository
git clone https://github.com/openztcc/openzt.git
cd openzt

# Build release DLL
./openzt.bat build --release

# Run with game launch
./openzt.bat run --release
```

## Running OpenZT

Build and run with `openzt.bat`:

```bash
# Standard build and run
./openzt.bat run --release

# Wait for game to exit
./openzt.bat run --release --wait

# Build only
./openzt.bat build --release
```

This copies the DLL to your Zoo Tycoon directory and launches the game.

### Lua Console

The console connects via TCP socket (port 8080) and doesn't need to be in the same directory:

```bash
# Open interactive console
./openzt.bat console

# Run single command and exit
./openzt.bat console --oneshot "help()"
```

See [CLAUDE.md](CLAUDE.md) for a complete list of console commands.

## Important Rules

### Assets Policy

**NEVER commit**:
- Zoo Tycoon assets (models, textures, sounds, etc.)
- Zoo Tycoon config files (zoo.ini, ai/*.ai, etc.)
- Decompiled or disassembled game code

OpenZT is a **clean-room reimplementation**. We do not use any original game code or assets.

**Mod assets** may be committed if:
- They are original creations (not derived from ZT or other games)
- You have permission from the creator
- Credit is given in a `CREDIT.md` file

### Code Style

- Use `./openzt.bat clippy` to check for linter warnings
- Follow existing Rust naming conventions
- Add comments for complex game memory structures

## Architecture

### Entry Point: lib.rs

The `lib.rs` file handles initialization and feature flags:

```rust
#[cfg(target_os = "windows")]
pub fn init() {
    #[cfg(feature = "integration-tests")]
    {
        integration_tests::init();
        return;
    }

    unsafe {
        zoo_init::init_detours().expect("Failed to initialize detours");
    }
}
```

### Module Pattern

Each feature module has an `init()` function called from `lib.rs`:

```rust
// In lib.rs
if cfg!(feature = "command-console") {
    command_console::init();
}
resource_manager::init();
expansions::init();
// ... etc
```

### Adding a New Module

1. Create `src/my_module.rs`
2. Add `mod my_module;` to `lib.rs`
3. Create an `init()` function
4. Call it behind a feature flag if needed

For complex modules, use a subdirectory:

```
src/
├── my_module/
│   ├── mod.rs
│   ├── submodule_a.rs
│   └── submodule_b.rs
```

## Patterns

### Structs: `#[repr(C)]`

All structs that mirror game memory must use `#[repr(C)]`:

```rust
#[derive(Debug)]
#[repr(C)]
pub struct GameStruct {
    vftable: u32,
    unknown_field: u32,
    string_field: ZTString,
    // ...
}
```

This prevents Rust from reordering fields for optimization, ensuring memory layout compatibility.

### Detours

Use the `openzt-detour` crate with procedural macros:

```rust
// 1. Define function signature in openzt-detour crate
// In openzt-detour/src/lib.rs:
use std::marker::PhantomData;
use openzt_detour_macros::FunctionDef;

pub const MY_GAME_FUNCTION: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> =
    FunctionDef { address: 0x00412345, function_type: PhantomData };
    // Address = Ghidra offset

// 2. Create detour in your module
use openzt_detour_macro::detour_mod;
use openzt_detour::MY_GAME_FUNCTION;

#[detour_mod]
pub mod my_module {
    use super::*;

    #[detour(MY_GAME_FUNCTION)]
    unsafe extern "thiscall" fn my_game_function_hook(param: u32) -> u32 {
        // Call original function
        MY_GAME_FUNCTION_DETOUR.call(param)
    }
}

// 3. Initialize in your module's init()
pub fn init() {
    unsafe { my_module::init_detours().unwrap() };
}
```

**Calling conventions** must match the original:
- `cdecl` - Most functions
- `stdcall` - Windows API style
- `fastcall` - First two args in ECX/EDX
- `thiscall` - C++ member functions (ECX = this)

### Calling Game Functions

Use `FunctionDef::original()` to get a function pointer:

```rust
let game_fn = unsafe { openzt_detour::MY_FUNCTION.original() };
let result = unsafe { game_fn(param1, param2) };
```

### Global State

Use `std::sync::LazyLock` for global state:

```rust
use std::sync::LazyLock;
use std::sync::Mutex;

static GLOBAL_STATE: LazyLock<Mutex<Vec<MyData>>> = LazyLock::new(|| {
    Mutex::new(Vec::new())
});

// Access
let mut data = GLOBAL_STATE.lock().unwrap();
data.push(MyData::new());
```

**Note**: The mutex may be overkill for single-threaded Zoo Tycoon, but provides future-proofing.

**Best practice**: Don't hold locks across other operations. Use wrapper functions:

```rust
pub fn add_data(item: MyData) {
    let mut data = GLOBAL_STATE.lock().unwrap();
    data.push(item);
    // Lock released here
}
```

### Lua Function Registration

Use the `lua_fn!` macro to register Lua functions from Rust:

```rust
// In your module's init()
lua_fn!("my_function", "Does something cool", "my_function(arg1, arg2)", |arg1: u32, arg2: String| {
    // Your code here
    Ok(format!("Result: {} {}", arg1, arg2))
});
```

### Resource Handlers

Register handlers for file types:

```rust
// In resource_manager initialization
resource_manager::add_handler(
    "bfb",  // Prefix/suffix match
    Box::new(BfbHandler::new())
);

// Handler implementation
pub struct BfbHandler;

impl Handler for BfbHandler {
    fn matches(&self, path: &PathBuf) -> bool {
        // Check if file matches
    }

    fn handle(&self, path: &PathBuf, file: &mut ZipFile) {
        // Process file
    }
}
```

## Feature Flags

Feature flags are defined in `Cargo.toml`:

```toml
[features]
default = ["experimental", "ini"]
release = []
ini = []
capture_ztlog = []
experimental = []
integration-tests = []
command-console = []
```

**Workflow**:
1. Start new features behind `experimental`
2. Move large features to their own flag when stable enough
3. Remove flag entirely when mature

Using feature flags:

```rust
if cfg!(feature = "my-feature") {
    info!("Feature 'my-feature' enabled");
    my_module::init();
}
```

## Testing

### Unit Tests

Standard Rust unit tests go in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        assert_eq!(2 + 2, 4);
    }
}
```

### Integration Tests

OpenZT includes a live-game integration test framework:

```bash
# Run all integration tests
./openzt.bat run --release -- --features integration-tests

# Run with wait for automation/CI
./openzt.bat run --release --wait -- --features integration-tests
```

Tests are located in `openzt/src/integration_tests/`:
- `patch_rollback.rs` - Test patch system error handling
- `loading_order.rs` - Test mod loading determinism

**Adding integration tests**:
1. Add test function to appropriate module
2. Create test resources in `resources/test/`
3. Use `include_str!()` / `include_bytes!()` for embedded resources

See [CLAUDE.md](CLAUDE.md) for detailed integration test documentation.

## Development Workflow

### Code Quality Checks

```bash
# Type checking
./openzt.bat check

# Linting
./openzt.bat clippy

# Unit tests
./openzt.bat test

# Documentation
./openzt.bat docs
```

`./openzt.bat check`/`build`/`run`/`crash-capture`/`debug-play` also run a detour-reentry audit
(`openzt/scripts/check-detour-reentry.sh`) automatically - it fails the build if any `FunctionDef` is both
`#[detour(NAME)]`'d and called via `NAME.original()` in the same file, which silently re-enters that detour
instead of reaching real vanilla in release builds. No separate command to run.

### Git Workflow

1. Create a feature branch
2. Make your changes
3. Run `./openzt.bat clippy` and fix warnings
4. Commit with descriptive messages
5. Create pull request

## Additional Resources

- [CLAUDE.md](CLAUDE.md) - Comprehensive developer guide
- [openzt.bat help](./openzt.bat) - Build script documentation
- [openzt-console README](./openzt-console/README.md) - Console details

## Questions?

- Open an issue on GitHub
- Ask in discussions
- Check existing issues and PRs for patterns
