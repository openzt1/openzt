# OpenZT Instance Manager Installation Script (Windows)
# This script installs the server and client binaries and sets up a Scheduled Task
# Must be run as Administrator

param (
    [switch]$ServerOnly,
    [switch]$CliOnly,
    [switch]$Help
)

if ($Help) {
    Write-Host "Usage: .\install.ps1 [-ServerOnly] [-CliOnly] [-Help]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -ServerOnly    Install only the server component"
    Write-Host "  -CliOnly       Install only the CLI client"
    Write-Host "  -Help          Show this help message"
    Write-Host ""
    Write-Host "If no option is specified, both server and CLI will be installed."
    exit 0
}

# Default: install both
$InstallServer = -not $CliOnly
$InstallCli    = -not $ServerOnly

# Paths
$InstallDir = "$env:ProgramFiles\OpenZT Instance Manager"
$ConfigDir  = "$env:ProgramData\openzt-instance-manager"
$ServiceName = "openzt-instance-manager"

# --- Helpers ---

function Write-Green($msg) { Write-Host $msg -ForegroundColor Green }
function Write-Yellow($msg) { Write-Host $msg -ForegroundColor Yellow }
function Write-Red($msg) { Write-Host $msg -ForegroundColor Red }

function Require-Admin {
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Write-Red "Error: This script must be run as Administrator."
        Write-Host "Right-click the script and select 'Run as administrator', or run from an elevated PowerShell."
        exit 1
    }
}

function Find-WorkspaceRoot($startDir) {
    $dir = $startDir
    while ($dir -ne $null) {
        $cargoToml = Join-Path $dir "Cargo.toml"
        if (Test-Path $cargoToml) {
            $content = Get-Content $cargoToml -Raw
            if ($content -match '(?m)^\[workspace\]') {
                return $dir
            }
        }
        $parent = Split-Path $dir -Parent
        if ($parent -eq $dir) { break }
        $dir = $parent
    }
    return $null
}

# --- Main ---

Require-Admin

Write-Green "OpenZT Instance Manager Installer"
Write-Host "=================================="
Write-Host ""

# Check Docker Desktop
Write-Host "Checking Docker..."
$dockerRunning = $false
try {
    $dockerInfo = docker info 2>&1
    if ($LASTEXITCODE -eq 0) { $dockerRunning = $true }
} catch {}

if (-not $dockerRunning) {
    # Try checking for Docker Desktop process as a fallback
    $dockerProcess = Get-Process "com.docker.proxy" -ErrorAction SilentlyContinue
    if ($dockerProcess) {
        $dockerRunning = $true
    }
}

if (-not $dockerRunning) {
    Write-Yellow "Warning: Docker Desktop is not running or not installed."
    Write-Host "The server requires Docker Desktop to manage Zoo Tycoon instances."
    Write-Host "Please install Docker Desktop from: https://www.docker.com/products/docker-desktop/"
    Write-Host ""
    Write-Yellow "Aborting installation. Start Docker Desktop and re-run this script."
    exit 1
}
Write-Green "Docker is running."
Write-Host ""

# Check Rust/Cargo
Write-Host "Checking for Rust/Cargo..."
$cargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargoCmd) {
    Write-Red "Error: cargo not found."
    Write-Host ""
    Write-Host "Rust is required to build the binaries. Please install Rust manually:"
    Write-Host "  1. Visit https://rustup.rs"
    Write-Host "  2. Download and run rustup-init.exe"
    Write-Host "  3. Follow the on-screen instructions (default installation is fine)"
    Write-Host "  4. Restart your terminal, then re-run this script"
    exit 1
}
Write-Green "Cargo found: $($cargoCmd.Source)"
Write-Host ""

# Find workspace root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkspaceRoot = Find-WorkspaceRoot $ScriptDir

if ($WorkspaceRoot) {
    Write-Green "Found workspace root: $WorkspaceRoot"
    Set-Location $WorkspaceRoot
} else {
    Write-Yellow "No workspace found; building from: $ScriptDir"
    Set-Location $ScriptDir
}

Write-Host ""
Write-Green "Building binaries..."
Write-Host "Working directory: $(Get-Location)"
Write-Host ""

if ($InstallServer) {
    Write-Host "Building server (openzt-instance-manager)..."
    cargo build --release --bin openzt-instance-manager
    if ($LASTEXITCODE -ne 0) {
        Write-Red "Error: Server build failed."
        exit 1
    }
}

if ($InstallCli) {
    Write-Host "Building CLI (openzt)..."
    cargo build --release --bin openzt --features cli
    if ($LASTEXITCODE -ne 0) {
        Write-Red "Error: CLI build failed."
        exit 1
    }
}

# Locate built binaries (workspace root or script dir)
$TargetDir = $null
$CwdTarget    = Join-Path (Get-Location) "target\release"
$ScriptTarget = Join-Path $ScriptDir "target\release"

$checkBin = if ($InstallServer) { "openzt-instance-manager.exe" } else { "openzt.exe" }
if (Test-Path (Join-Path $CwdTarget $checkBin)) {
    $TargetDir = $CwdTarget
} elseif (Test-Path (Join-Path $ScriptTarget $checkBin)) {
    $TargetDir = $ScriptTarget
} else {
    Write-Red "Error: Cannot find built binaries."
    Write-Host "Searched in:"
    Write-Host "  $CwdTarget"
    Write-Host "  $ScriptTarget"
    Write-Host ""
    Write-Host "Please check if the build succeeded."
    exit 1
}

Write-Host ""
Write-Green "Installing binaries from: $TargetDir"

# Verify binaries
if ($InstallServer -and -not (Test-Path "$TargetDir\openzt-instance-manager.exe")) {
    Write-Red "Error: openzt-instance-manager.exe not found in $TargetDir"
    exit 1
}
if ($InstallCli -and -not (Test-Path "$TargetDir\openzt.exe")) {
    Write-Red "Error: openzt.exe not found in $TargetDir"
    exit 1
}

# Stop existing scheduled task if present (must stop BEFORE copying binaries)
if ($InstallServer) {
    $existingTask = Get-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
    if ($existingTask) {
        Write-Yellow "Existing scheduled task found; stopping it first..."
        Stop-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
        # Wait a moment for the process to fully stop
        Start-Sleep -Seconds 1
        Write-Green "Server stopped."
    }
}

# Create install dir
Write-Host ""
Write-Green "Creating install directory: $InstallDir"
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

# Copy binaries
if ($InstallServer) {
    Copy-Item "$TargetDir\openzt-instance-manager.exe" "$InstallDir\" -Force
    Write-Green "Installed: $InstallDir\openzt-instance-manager.exe"
}
if ($InstallCli) {
    Copy-Item "$TargetDir\openzt.exe" "$InstallDir\" -Force
    Write-Green "Installed: $InstallDir\openzt.exe"
}

# Create config dir
Write-Host ""
Write-Green "Creating config directory: $ConfigDir"
New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null

# Copy config.toml
$ConfigSource = $null
if (Test-Path "$ScriptDir\config.toml") {
    $ConfigSource = "$ScriptDir\config.toml"
} elseif (Test-Path "config.toml") {
    $ConfigSource = (Resolve-Path "config.toml").Path
}

if ($ConfigSource) {
    Copy-Item $ConfigSource "$ConfigDir\config.toml" -Force
    Write-Host "Copied config from: $ConfigSource"
} else {
    Write-Yellow "Warning: config.toml not found. The server will use defaults."
    Write-Yellow "Place your config.toml in: $ConfigDir"
}

# Add install dir to system PATH (if not already present)
Write-Host ""
Write-Green "Updating system PATH..."
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "Machine")
if ($currentPath -split ";" -contains $InstallDir) {
    Write-Host "PATH already contains: $InstallDir (skipped)"
} else {
    [Environment]::SetEnvironmentVariable("PATH", "$currentPath;$InstallDir", "Machine")
    Write-Green "Added to system PATH: $InstallDir"
    Write-Yellow "Note: Open a new terminal for PATH changes to take effect."
}

if ($InstallServer) {
    # Enable Task Scheduler history (disabled by default on Windows)
    Write-Host "Enabling Task Scheduler history..."
    wevtutil set-log Microsoft-Windows-TaskScheduler/Operational /enabled:true | Out-Null
    Write-Green "Task Scheduler history enabled."

    # Create a launcher script.
    # Redirects server stdout/stderr to a log file so output is not silently discarded
    # when running as a Scheduled Task. Task Scheduler itself handles WorkingDirectory.
    Write-Host ""
    Write-Green "Creating server launcher script..."
    $LauncherPath = "$InstallDir\start-server.ps1"
    $launcherContent = @"
# OpenZT Instance Manager - Server Launcher
# Captures stdout/stderr to a log file. Overwritten on each start.
Start-Process ``
    -FilePath "$InstallDir\openzt-instance-manager.exe" ``
    -WorkingDirectory "$ConfigDir" ``
    -RedirectStandardOutput "$ConfigDir\server.log" ``
    -RedirectStandardError "$ConfigDir\server-error.log" ``
    -NoNewWindow ``
    -Wait
"@
    Set-Content -Path $LauncherPath -Value $launcherContent -Encoding UTF8
    Write-Green "Created: $LauncherPath"

    # Remove existing scheduled task registration if present (task already stopped above)
    $existingTask = Get-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
    if ($existingTask) {
        Write-Yellow "Removing existing scheduled task registration..."
        Unregister-ScheduledTask -TaskName $ServiceName -Confirm:$false
        Write-Green "Task removed."
    }

    # Register a Scheduled Task.
    # Task Scheduler is used instead of a Windows Service because the binary reads
    # config.toml from its working directory, and New-ScheduledTaskAction supports
    # -WorkingDirectory natively. Windows services have no equivalent for this.
    Write-Host ""
    Write-Green "Registering Scheduled Task: $ServiceName"

    $action = New-ScheduledTaskAction `
        -Execute "powershell.exe" `
        -Argument "-WindowStyle Hidden -NonInteractive -NoProfile -ExecutionPolicy Bypass -File `"$LauncherPath`"" `
        -WorkingDirectory $ConfigDir

    # Create multiple triggers: at startup and at logon
    $trigger1 = New-ScheduledTaskTrigger -AtStartup
    $trigger2 = New-ScheduledTaskTrigger -AtLogon
    $triggers = @($trigger1, $trigger2)

    $settings = New-ScheduledTaskSettingsSet `
        -ExecutionTimeLimit ([TimeSpan]::Zero) `
        -RestartCount 3 `
        -RestartInterval (New-TimeSpan -Minutes 1) `
        -MultipleInstances IgnoreNew

    $principal = New-ScheduledTaskPrincipal `
        -UserId $env:USERNAME `
        -LogonType Interactive `
        -RunLevel Highest

    Register-ScheduledTask `
        -TaskName $ServiceName `
        -Description "OpenZT Instance Manager API Server" `
        -Action $action `
        -Trigger $triggers `
        -Settings $settings `
        -Principal $principal `
        -Force | Out-Null

    Write-Green "Scheduled task registered: $ServiceName"

    # Start the scheduled task
    Write-Host "Starting server..."
    Start-ScheduledTask -TaskName $ServiceName
    Write-Green "Server started via scheduled task."
}

# Summary
Write-Host ""
Write-Green "Installation complete!"
Write-Host ""
Write-Host "Binaries installed:"
if ($InstallServer) { Write-Host "  - $InstallDir\openzt-instance-manager.exe (server)" }
if ($InstallCli)    { Write-Host "  - $InstallDir\openzt.exe (client)" }
Write-Host ""

if ($InstallServer) {
    Write-Host "Config directory: $ConfigDir"
    Write-Host ""
    Write-Host "To start the server:"
    Write-Host "  Start-ScheduledTask -TaskName $ServiceName"
    Write-Host ""
    Write-Host "To stop the server:"
    Write-Host "  Stop-ScheduledTask -TaskName $ServiceName"
    Write-Host ""
    Write-Host "The task is configured to start automatically at boot."
    Write-Host "To disable auto-start:"
    Write-Host "  Disable-ScheduledTask -TaskName $ServiceName"
    Write-Host ""
    Write-Host "To check task status:"
    Write-Host "  Get-ScheduledTask -TaskName $ServiceName"
    Write-Host ""
    Write-Host "To view server logs:"
    Write-Host "  Get-Content '$ConfigDir\server.log'"
    Write-Host "  Get-Content '$ConfigDir\server-error.log'"
    Write-Host "  (logs are overwritten each time the server starts)"
    Write-Host ""
    Write-Host "To follow logs in real time:"
    Write-Host "  Get-Content '$ConfigDir\server.log' -Wait"
    Write-Host ""
}

if ($InstallCli) {
    Write-Green "CLI usage examples (open a new terminal first for PATH to take effect):"
    Write-Host "  openzt health                       # Check API health"
    Write-Host "  openzt list                         # List instances"
    Write-Host "  openzt create <path-to-dll>         # Create instance"
    Write-Host "  openzt get <instance-id>            # Get instance details"
    Write-Host "  openzt logs <instance-id>           # Get instance logs"
    Write-Host "  openzt delete <instance-id>         # Delete instance"
    Write-Host ""
    Write-Host "Use 'openzt --help' for more information."
    Write-Host ""
}

if (-not $InstallServer -and $InstallCli) {
    Write-Yellow "Note: Only the CLI was installed."
    Write-Host "The CLI requires a running server. Make sure the server is installed and running"
    Write-Host "on a reachable host, or set the OPENZT_API_URL environment variable:"
    Write-Host "  `$env:OPENZT_API_URL = 'http://your-server:3000'"
    Write-Host ""
}
