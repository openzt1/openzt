@echo off
setlocal enabledelayedexpansion

REM OpenZT Unified Build Script
REM Combines build, run, and docs functionality

REM ============================================================
REM Main Dispatcher
REM ============================================================

IF "%~1"=="" GOTO show_help
IF "%~1"=="help" GOTO show_help
IF "%~1"=="--help" GOTO show_help
IF "%~1"=="-h" GOTO show_help
IF "%~1"=="build" GOTO parse_build_flags
IF "%~1"=="run" GOTO parse_run_flags
IF "%~1"=="docs" GOTO docs
IF "%~1"=="console" GOTO console
IF "%~1"=="check" GOTO check
IF "%~1"=="clippy" GOTO clippy
IF "%~1"=="test" GOTO test
IF "%~1"=="integration-tests" GOTO integration_tests
IF "%~1"=="crash-capture" GOTO crash_capture
IF "%~1"=="debug-play" GOTO debug_play
IF "%~1"=="update" GOTO update
IF "%~1"=="tree" GOTO tree

echo Error: Unknown subcommand "%~1"
echo.
GOTO show_help

REM ============================================================
REM Parse Build Flags
REM ============================================================

:parse_build_flags
SET RUN_AFTER_BUILD=
SHIFT
GOTO parse_flags

:parse_run_flags
SET RUN_AFTER_BUILD=1
SHIFT
GOTO parse_flags

REM ============================================================
REM Integration Tests Command
REM ============================================================

:integration_tests
SET RUN_AFTER_BUILD=1
SET RELEASE_FLAG=1
SET WAIT_FLAG=1
SET CARGO_ARGS=--features integration-tests
SET INTEGRATION_TESTS_MODE=1
SHIFT
GOTO build

:parse_flags
SET RELEASE_FLAG=
SET TEST_FLAG=
SET WAIT_FLAG=
SET CARGO_ARGS=
SET PARSING_CARGO_ARGS=

:parse_loop
IF "%~1"=="" GOTO validate_and_build
IF "%~1"=="--release" (
    SET RELEASE_FLAG=1
    SHIFT
    GOTO parse_loop
)
IF "%~1"=="--test" (
    SET TEST_FLAG=1
    SHIFT
    GOTO parse_loop
)
IF "%~1"=="--wait" (
    SET WAIT_FLAG=1
    SHIFT
    GOTO parse_loop
)
IF "%~1"=="--" (
    SET PARSING_CARGO_ARGS=1
    SHIFT
    GOTO parse_loop
)
IF DEFINED PARSING_CARGO_ARGS (
    SET CARGO_ARGS=!CARGO_ARGS! %~1
    SHIFT
    GOTO parse_loop
)
echo Error: Unknown flag "%~1"
exit /b 1

:validate_and_build
GOTO build

REM ============================================================
REM Build Function
REM ============================================================

:build
CALL :audit_detour_reentry
IF !errorlevel! NEQ 0 exit /b !errorlevel!

REM Set manifest path and DLL name
IF DEFINED TEST_FLAG (
    SET MANIFEST_PATH=openzt-test-dll/Cargo.toml
    SET DLL_NAME=openzttest.dll
    SET RUST_BACKTRACE=1
) ELSE (
    SET MANIFEST_PATH=openzt-dll/Cargo.toml
    SET DLL_NAME=openzt.dll
)

REM Set build type
SET BUILD_TYPE=debug
SET BUILD_FLAGS=
IF DEFINED RELEASE_FLAG (
    SET BUILD_TYPE=release
    SET BUILD_FLAGS=--release
)

REM Set feature flags
SET FEATURE_FLAGS=--features "command-console"
IF DEFINED TEST_FLAG (
    SET FEATURE_FLAGS=
)

REM Display build info
echo Building !DLL_NAME! (!BUILD_TYPE!)...
IF DEFINED FEATURE_FLAGS (
    echo Features: !FEATURE_FLAGS!
)

REM Execute cargo build for DLL
cargo build --manifest-path !MANIFEST_PATH! --lib --target=i686-pc-windows-msvc !BUILD_FLAGS! !FEATURE_FLAGS! !CARGO_ARGS!

IF !errorlevel! NEQ 0 (
    echo.
    echo Build failed
    pause
    exit /b !errorlevel!
)

echo.
echo Build successful: target\i686-pc-windows-msvc\!BUILD_TYPE!\!DLL_NAME!

REM Write state file on success
echo BUILD_TYPE=!BUILD_TYPE! > .openzt-build-state
echo DLL_NAME=!DLL_NAME! >> .openzt-build-state

REM If run command was used, continue to copy and launch
IF DEFINED RUN_AFTER_BUILD GOTO copy_and_run
GOTO :EOF

REM ============================================================
REM Check if Zoo Tycoon is Already Running
REM ============================================================

:check_zoo_running
REM Check if zoo.exe is already running using PowerShell (avoids pipe issues)
powershell -Command "Get-Process -Name zoo -ErrorAction SilentlyContinue" >NUL 2>&1
IF "%ERRORLEVEL%"=="0" (
    echo.
    echo ERROR: Zoo Tycoon is already running.
    echo Please close the existing instance before launching a new one.
    echo.
    exit /b 1
)
exit /b 0

REM ============================================================
REM Detour-Reentry Audit
REM ============================================================
REM Fails if any FunctionDef name is both #[detour(NAME)]'d and called via NAME.original() in the same
REM file - .original() on a hooked address silently re-enters that detour in release builds (see
REM openzt/scripts/check-detour-reentry.sh's own header comment for the full history/reasoning).

:audit_detour_reentry
REM Prefer Git for Windows' own bash.exe by well-known install path rather than trusting `where bash` -
REM on a machine with WSL installed, `where bash` can resolve to C:\Windows\System32\bash.exe (the WSL
REM launcher) instead, which doesn't understand a raw Windows path with backslashes and fails with
REM "No such file or directory" on the script path itself.
SET GIT_BASH=
IF EXIST "%ProgramFiles%\Git\bin\bash.exe" SET GIT_BASH=%ProgramFiles%\Git\bin\bash.exe
IF NOT DEFINED GIT_BASH IF EXIST "%ProgramFiles(x86)%\Git\bin\bash.exe" SET GIT_BASH=%ProgramFiles(x86)%\Git\bin\bash.exe
IF NOT DEFINED GIT_BASH (
    where bash >nul 2>nul
    IF !errorlevel! NEQ 0 (
        echo Warning: bash not found - skipping detour-reentry audit ^(install Git for Windows or add its bin\ to PATH^)
        exit /b 0
    )
    SET GIT_BASH=bash
)
"!GIT_BASH!" "%~dp0openzt\scripts\check-detour-reentry.sh"
exit /b !errorlevel!

REM ============================================================
REM Copy and Run Function
REM ============================================================

:copy_and_run
REM BUILD_TYPE and DLL_NAME are already set from the build step
REM Set source path
SET SOURCE_DLL=target\i686-pc-windows-msvc\!BUILD_TYPE!\!DLL_NAME!

REM Check source exists
IF NOT EXIST "!SOURCE_DLL!" (
    echo Error: Built DLL not found at !SOURCE_DLL!
    pause
    exit /b 1
)

REM Standard DLL copy method
REM Check for already running game before attempting to copy
CALL :check_zoo_running
IF !errorlevel! NEQ 0 exit /b !errorlevel!

REM Delete old DLLs
echo.
echo Cleaning up old DLLs...
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzt.dll" 2>nul
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openztrpc.dll" 2>nul
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzttest.dll" 2>nul

REM Determine destination name
IF !DLL_NAME!==openzt.dll (
    SET DEST_NAME=res-openzt.dll
) ELSE (
    SET DEST_NAME=res-openzttest.dll
)

REM Copy DLL
echo Copying !DLL_NAME! to Zoo Tycoon directory...
copy "!SOURCE_DLL!" "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\!DEST_NAME!"

IF !errorlevel! NEQ 0 (
    echo.
    echo Copy failed
    pause
    exit /b !errorlevel!
)

REM Launch game
echo.
IF DEFINED WAIT_FLAG (
    echo Launching Zoo Tycoon and waiting for exit...
    start "Zoo Tycoon" /WAIT "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\zoo.exe"
    echo.
    echo Zoo Tycoon has exited.

    REM Display integration test results if in integration tests mode
    IF DEFINED INTEGRATION_TESTS_MODE (
        echo.
        echo ============================================================
        echo Integration Test Results
        echo ============================================================
        powershell -Command "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-Content -Path 'C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\openzt_integration_tests.log' | Out-Host"
        echo.
        echo ============================================================
        echo Full logs available at:
        echo   "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\openzt.log"
        echo ============================================================
        echo.
    )
) ELSE (
    echo Launching Zoo Tycoon...
    start "Zoo Tycoon" "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\zoo.exe"
)

GOTO :EOF

REM ============================================================
REM Crash Capture Function
REM ============================================================
REM Builds the test DLL (release), copies it in as res-openzttest.dll, then launches Zoo Tycoon
REM directly under cdb non-interactively (-G, no initial breakpoint stop) so a crash that
REM reproduces automatically (e.g. off the reimplementation-tests battery, no manual play needed)
REM is caught, its register/stack state dumped, and the process quit - all without a debugger
REM window to babysit. See openzt/plans/ztshowscriptmgr-open-items.md item 10 for the technique
REM this automates.

:crash_capture
SHIFT
SET CRASH_LOG=crash_capture_output.txt

:crash_capture_args_loop
IF "%~1"=="" GOTO run_crash_capture
IF "%~1"=="--out" GOTO crash_capture_out_flag
echo Error: Unknown flag "%~1" for crash-capture
exit /b 1

:crash_capture_out_flag
SHIFT
SET CRASH_LOG=%~1
SHIFT
GOTO crash_capture_args_loop

:run_crash_capture
CALL :audit_detour_reentry
IF !errorlevel! NEQ 0 exit /b !errorlevel!

echo Building openzttest.dll (release) for crash capture...
cargo build --manifest-path openzt-test-dll/Cargo.toml --lib --target=i686-pc-windows-msvc --release

IF !errorlevel! NEQ 0 (
    echo.
    echo Build failed
    exit /b !errorlevel!
)

SET SOURCE_DLL=target\i686-pc-windows-msvc\release\openzttest.dll
IF NOT EXIST "!SOURCE_DLL!" (
    echo Error: Built DLL not found at !SOURCE_DLL!
    exit /b 1
)

CALL :check_zoo_running
IF !errorlevel! NEQ 0 exit /b !errorlevel!

echo.
echo Cleaning up old DLLs...
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzt.dll" 2>nul
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openztrpc.dll" 2>nul
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzttest.dll" 2>nul

echo Copying openzttest.dll to Zoo Tycoon directory...
copy "!SOURCE_DLL!" "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzttest.dll"

IF !errorlevel! NEQ 0 (
    echo.
    echo Copy failed
    exit /b !errorlevel!
)

SET CDB_EXE=
IF EXIST "C:\Program Files (x86)\Windows Kits\10\Debuggers\x86\cdb.exe" SET CDB_EXE=C:\Program Files (x86)\Windows Kits\10\Debuggers\x86\cdb.exe
IF NOT DEFINED CDB_EXE IF EXIST "C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe" SET CDB_EXE=C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe
IF NOT DEFINED CDB_EXE (
    echo Error: cdb.exe not found under "C:\Program Files (x86)\Windows Kits\10\Debuggers".
    echo Install the "Debugging Tools for Windows" component of the Windows SDK.
    exit /b 1
)

echo.
echo Launching Zoo Tycoon under cdb ^(non-interactive^) - runs to completion or crash, then quits...
echo Output: !CRASH_LOG!
"!CDB_EXE!" -G -c "g;kv;r;q" "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\zoo.exe" > "!CRASH_LOG!" 2>&1

echo.
echo Done. Full output written to !CRASH_LOG!
echo Tail:
echo ------------------------------------------------------------
powershell -Command "Get-Content -Path '!CRASH_LOG!' -Tail 20"
echo ------------------------------------------------------------

GOTO :EOF

REM ============================================================
REM Debug Play Function
REM ============================================================
REM Builds the REAL (non-test) openzt.dll, launches Zoo Tycoon under cdb with proper .pdb
REM symbols loaded, then lets it run interactively (the game window is fully playable - cdb just
REM watches silently) until either the process exits normally or an unhandled exception occurs,
REM at which point a real, symbolized stack trace and register dump is written to the log and cdb
REM quits. Use this instead of `crash-capture` for a bug that needs manual play to reproduce
REM (crash-capture's own game runs under the test DLL, which auto-runs the reimplementation-tests
REM battery and exits immediately - it never reaches an interactive, playable state at all).
REM See CLAUDE.md's "Getting real symbols from a live crash" section for why the symbol path and
REM module-name handling below are necessary, and for the gotchas around private/inlined functions
REM not always resolving to their real name.

:debug_play
SHIFT
SET CRASH_LOG=debug_play_output.txt
SET RELEASE_FLAG=

:debug_play_args_loop
IF "%~1"=="" GOTO run_debug_play
IF "%~1"=="--out" GOTO debug_play_out_flag
IF "%~1"=="--release" (
    SET RELEASE_FLAG=1
    SHIFT
    GOTO debug_play_args_loop
)
echo Error: Unknown flag "%~1" for debug-play
exit /b 1

:debug_play_out_flag
SHIFT
SET CRASH_LOG=%~1
SHIFT
GOTO debug_play_args_loop

:run_debug_play
CALL :audit_detour_reentry
IF !errorlevel! NEQ 0 exit /b !errorlevel!

SET BUILD_TYPE=debug
SET BUILD_FLAGS=
IF DEFINED RELEASE_FLAG (
    SET BUILD_TYPE=release
    SET BUILD_FLAGS=--release
)

echo Building openzt.dll (!BUILD_TYPE!) for debug-play...
cargo build --manifest-path openzt-dll/Cargo.toml --lib --target=i686-pc-windows-msvc !BUILD_FLAGS! --features "command-console"

IF !errorlevel! NEQ 0 (
    echo.
    echo Build failed
    exit /b !errorlevel!
)

SET SOURCE_DLL=target\i686-pc-windows-msvc\!BUILD_TYPE!\openzt.dll
IF NOT EXIST "!SOURCE_DLL!" (
    echo Error: Built DLL not found at !SOURCE_DLL!
    exit /b 1
)

CALL :check_zoo_running
IF !errorlevel! NEQ 0 exit /b !errorlevel!

echo.
echo Cleaning up old DLLs...
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzt.dll" 2>nul
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openztrpc.dll" 2>nul
del "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzttest.dll" 2>nul

echo Copying openzt.dll to Zoo Tycoon directory...
copy "!SOURCE_DLL!" "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\res-openzt.dll"

IF !errorlevel! NEQ 0 (
    echo.
    echo Copy failed
    exit /b !errorlevel!
)

SET CDB_EXE=
IF EXIST "C:\Program Files (x86)\Windows Kits\10\Debuggers\x86\cdb.exe" SET CDB_EXE=C:\Program Files (x86)\Windows Kits\10\Debuggers\x86\cdb.exe
IF NOT DEFINED CDB_EXE IF EXIST "C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe" SET CDB_EXE=C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe
IF NOT DEFINED CDB_EXE (
    echo Error: cdb.exe not found under "C:\Program Files (x86)\Windows Kits\10\Debuggers".
    echo Install the "Debugging Tools for Windows" component of the Windows SDK.
    exit /b 1
)

REM `res-openzt`'s own PDB sits next to the DLL in this same target directory - added ahead of
REM `srv*` so cdb finds our own symbols first, falling back to Microsoft's symbol server for
REM everything else (ntdll, kernel32, ...).
SET SYM_PATH=srv*;%CD%\target\i686-pc-windows-msvc\!BUILD_TYPE!

echo.
echo Launching Zoo Tycoon under cdb - the window is fully playable, cdb watches silently.
echo Reproduce the bug now; a crash will symbolize automatically and quit cdb.
echo Output: !CRASH_LOG!
"!CDB_EXE!" -G -y "!SYM_PATH!" -c "sxe ld:res-openzt;g;.reload /f res-openzt.dll;g;kv;r;q" "C:\Program Files (x86)\Microsoft Games\Zoo Tycoon\zoo.exe" > "!CRASH_LOG!" 2>&1

echo.
echo Done. Full output written to !CRASH_LOG!
echo Tail:
echo ------------------------------------------------------------
powershell -Command "Get-Content -Path '!CRASH_LOG!' -Tail 25"
echo ------------------------------------------------------------

GOTO :EOF

REM ============================================================
REM Docs Function
REM ============================================================

:docs
echo Opening documentation...
cargo rustdoc --manifest-path openzt/Cargo.toml --lib --target i686-pc-windows-msvc --open -- --document-private-items

IF !errorlevel! NEQ 0 (
    echo.
    echo Documentation generation failed
    pause
    exit /b !errorlevel!
)

GOTO :EOF

REM ============================================================
REM Console Function
REM ============================================================

:console
SHIFT
SET CONSOLE_ARGS=
:console_args_loop
IF "%~1"=="" GOTO run_console
SET CONSOLE_ARGS=!CONSOLE_ARGS! %1
SHIFT
GOTO console_args_loop

:run_console
IF "!CONSOLE_ARGS!"=="" (
    echo Opening console...
    cargo run --manifest-path=openzt-console/Cargo.toml
) ELSE (
    echo Running console command...
    cargo run --manifest-path=openzt-console/Cargo.toml --!CONSOLE_ARGS!
)

IF !errorlevel! NEQ 0 (
    echo.
    echo Console failed
    pause
    exit /b !errorlevel!
)

GOTO :EOF

REM ============================================================
REM Check Function
REM ============================================================

:check
SHIFT
SET CHECK_ARGS=
SET CHECK_MANIFEST=openzt/Cargo.toml
:check_args_loop
IF "%~1"=="" GOTO run_check
IF "%~1"=="--test" (
    SET CHECK_MANIFEST=openzt-test-dll/Cargo.toml
    SHIFT
    GOTO check_args_loop
)
SET CHECK_ARGS=!CHECK_ARGS! %1
SHIFT
GOTO check_args_loop

:run_check
CALL :audit_detour_reentry
IF !errorlevel! NEQ 0 exit /b !errorlevel!

echo Running cargo check on !CHECK_MANIFEST!...
cargo check --manifest-path !CHECK_MANIFEST! --target i686-pc-windows-msvc !CHECK_ARGS!

IF !errorlevel! NEQ 0 (
    echo.
    echo Cargo check failed
    pause
    exit /b !errorlevel!
)

echo.
echo Cargo check passed
GOTO :EOF

REM ============================================================
REM Clippy Function
REM ============================================================

:clippy
SHIFT
SET CLIPPY_ARGS=
SET CLIPPY_MANIFEST=openzt/Cargo.toml
:clippy_args_loop
IF "%~1"=="" GOTO run_clippy
IF "%~1"=="--test" (
    SET CLIPPY_MANIFEST=openzt-test-dll/Cargo.toml
    SHIFT
    GOTO clippy_args_loop
)
SET CLIPPY_ARGS=!CLIPPY_ARGS! %1
SHIFT
GOTO clippy_args_loop

:run_clippy
echo Running cargo clippy on !CLIPPY_MANIFEST!...
cargo clippy --manifest-path !CLIPPY_MANIFEST! --target i686-pc-windows-msvc !CLIPPY_ARGS!

IF !errorlevel! NEQ 0 (
    echo.
    echo Clippy found issues
    pause
    exit /b !errorlevel!
)

echo.
echo Clippy passed
GOTO :EOF

REM ============================================================
REM Test Function
REM ============================================================

:test
SHIFT
SET TEST_ARGS=
:test_args_loop
IF "%~1"=="" GOTO run_test
SET TEST_ARGS=!TEST_ARGS! %1
SHIFT
GOTO test_args_loop

:run_test
echo Running cargo test on openzt...
cargo test --manifest-path openzt/Cargo.toml --target i686-pc-windows-msvc !TEST_ARGS!

IF !errorlevel! NEQ 0 (
    echo.
    echo Tests failed
    pause
    exit /b !errorlevel!
)

echo.
echo Tests passed
GOTO :EOF

REM ============================================================
REM Update Function
REM ============================================================

:update
SHIFT
SET UPDATE_ARGS=
:update_args_loop
IF "%~1"=="" GOTO run_update
SET UPDATE_ARGS=!UPDATE_ARGS! %1
SHIFT
GOTO update_args_loop

:run_update
echo Running cargo update on workspace...
cargo update --workspace !UPDATE_ARGS!

IF !errorlevel! NEQ 0 (
    echo.
    echo Cargo update failed
    pause
    exit /b !errorlevel!
)

echo.
echo Cargo update complete
GOTO :EOF

REM ============================================================
REM Tree Function
REM ============================================================

:tree
SHIFT
SET TREE_ARGS=
:tree_args_loop
IF "%~1"=="" GOTO run_tree
SET TREE_ARGS=!TREE_ARGS! %1
SHIFT
GOTO tree_args_loop

:run_tree
cargo tree --workspace !TREE_ARGS!

IF !errorlevel! NEQ 0 (
    echo.
    echo Cargo tree failed
    pause
    exit /b !errorlevel!
)

GOTO :EOF

REM ============================================================
REM Help Function
REM ============================================================

:show_help
echo OpenZT Build Script
echo.
echo Usage: openzt.bat ^<subcommand^> [flags] [-- cargo-args]
echo.
echo Subcommands:
echo   build              Build the DLL only
echo   run                Build the DLL and launch the game
echo   check              Run cargo check on openzt crate (pass --test to check openzt-test-dll instead)
echo   clippy             Run cargo clippy on openzt crate (pass --test to check openzt-test-dll instead)
echo   test               Run cargo test on openzt crate
echo   integration-tests  Run integration tests (builds release, launches game, displays results)
echo   crash-capture      Build test DLL, launch game under cdb non-interactively, dump crash info (--out ^<file^>)
echo   debug-play         Build real DLL, launch game under cdb WITH symbols, play manually, auto-dump on crash (--release, --out ^<file^>)
echo   update             Run cargo update on the workspace (forwards extra args, e.g. -p ^<pkg^>)
echo   tree               Run cargo tree on the workspace (forwards extra args, e.g. -i ^<pkg^>)
echo   docs               Generate and open documentation
echo   console            Open interactive Lua console or run oneshot command
echo   help               Show this help message
echo.
echo Build/Run Flags:
echo   --release      Build with release optimizations
echo   --test         Build the test DLL (openzt-test-dll)
echo   --wait         Wait for Zoo Tycoon to exit before returning
echo   -- ^<args^>      Forward additional arguments to cargo
echo.
echo Note: command-console feature is enabled by default for non-test builds.
echo.
echo Examples:
echo   openzt.bat build                     Build debug DLL with command-console
echo   openzt.bat build --release           Build release DLL with command-console
echo   openzt.bat run                       Build debug, copy DLL, launch game
echo   openzt.bat run --release             Build release, copy DLL, launch game
echo   openzt.bat run --test                Build test DLL and launch game
echo   openzt.bat check                     Run cargo check on openzt
echo   openzt.bat clippy                    Run cargo clippy on openzt
echo   openzt.bat test                      Run cargo test on openzt
echo   openzt.bat test -- --nocapture        Run cargo test, forwarding extra args to cargo
echo   openzt.bat integration-tests         Run integration tests (builds release, displays results)
echo   openzt.bat crash-capture             Build test DLL, run under cdb, dump crash info
echo   openzt.bat crash-capture --out x.txt Same, writing output to a custom file
echo   openzt.bat debug-play                Build debug DLL, play interactively under cdb, symbolized crash dump
echo   openzt.bat debug-play --release       Same, release build
echo   openzt.bat docs                      Generate and open docs
echo   openzt.bat console                   Open interactive Lua console
echo   openzt.bat console --oneshot "help()"          Run single Lua command and exit
echo   openzt.bat console --oneshot "add_cash(10000)" Add cash via oneshot command
echo   openzt.bat run --wait                Build debug, launch game, wait for exit
echo   openzt.bat run --release --wait      Build release, launch game, wait for exit
echo.
GOTO :EOF
