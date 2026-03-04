# OpenZT Instance Manager Uninstallation Script (Windows)
# Must be run as Administrator

$InstallDir  = "$env:ProgramFiles\OpenZT Instance Manager"
$ConfigDir   = "$env:ProgramData\openzt-instance-manager"
$ServiceName = "openzt-instance-manager"

function Write-Green($msg)  { Write-Host $msg -ForegroundColor Green }
function Write-Yellow($msg) { Write-Host $msg -ForegroundColor Yellow }
function Write-Red($msg)    { Write-Host $msg -ForegroundColor Red }

# Require Administrator
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Red "Error: This script must be run as Administrator."
    Write-Host "Right-click the script and select 'Run as administrator', or run from an elevated PowerShell."
    exit 1
}

Write-Red "OpenZT Instance Manager Uninstaller"
Write-Host "====================================="
Write-Host ""

# Stop and remove scheduled task
$task = Get-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
if ($task) {
    if ($task.State -eq "Running") {
        Write-Yellow "Stopping task: $ServiceName..."
        Stop-ScheduledTask -TaskName $ServiceName -ErrorAction SilentlyContinue
    }
    Write-Yellow "Removing scheduled task: $ServiceName..."
    Unregister-ScheduledTask -TaskName $ServiceName -Confirm:$false
    Write-Host "Scheduled task removed."
} else {
    Write-Host "Scheduled task '$ServiceName' not found (skipped)."
}

# Remove install directory (binaries)
if (Test-Path $InstallDir) {
    Write-Yellow "Removing install directory: $InstallDir..."
    Remove-Item -Path $InstallDir -Recurse -Force
    Write-Host "Removed: $InstallDir"
} else {
    Write-Host "Install directory not found (skipped): $InstallDir"
}

# Remove install dir from system PATH
Write-Yellow "Updating system PATH..."
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "Machine")
$pathParts = $currentPath -split ";" | Where-Object { $_ -ne $InstallDir -and $_ -ne "" }
$newPath = $pathParts -join ";"
if ($newPath -ne $currentPath) {
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "Machine")
    Write-Host "Removed from system PATH: $InstallDir"
} else {
    Write-Host "PATH entry not found (skipped): $InstallDir"
}

Write-Host ""
Write-Green "Uninstallation complete!"
Write-Host ""
Write-Host "The following have been removed:"
Write-Host "  - $InstallDir (binaries)"
Write-Host "  - Scheduled Task: $ServiceName"
Write-Host "  - PATH entry: $InstallDir"
Write-Host ""
Write-Yellow "Note: Config directory was preserved: $ConfigDir"
Write-Host "To remove it manually, run:"
Write-Host "  Remove-Item -Path '$ConfigDir' -Recurse -Force"
Write-Host ""
