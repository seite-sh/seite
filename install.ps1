# install.ps1 — Install the seite static site generator on Windows
#
# Usage:
#   irm https://seite.sh/install.ps1 | iex
#
# Options (via environment variables):
#   $env:VERSION     Pin to a specific release (e.g., $env:VERSION = "v0.1.0")
#   $env:INSTALL_DIR Override install location (default: ~\.local\bin)

$ErrorActionPreference = "Stop"

$Repo = "seite-sh/seite"
$Binary = "seite.exe"
$DefaultInstallDir = Join-Path $HOME ".local\bin"
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { $DefaultInstallDir }

function Write-Info($msg) { Write-Host "info  $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "warn  $msg" -ForegroundColor Yellow }
function Write-Err($msg) { Write-Host "error $msg" -ForegroundColor Red }

# --- Detect architecture ---
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" { $Target = "aarch64-pc-windows-msvc" }
    default {
        Write-Err "Unsupported architecture: $Arch"
        Write-Host "  Install from source instead: cargo install seite"
        exit 1
    }
}

# --- Resolve version ---
if ($env:VERSION) {
    $Version = $env:VERSION
    if (-not $Version.StartsWith("v")) { $Version = "v$Version" }
} else {
    Write-Info "Fetching latest release..."
    $Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $Release.tag_name
    if (-not $Version) {
        Write-Err "Could not determine latest release version."
        Write-Host '  Try pinning a version: $env:VERSION = "v0.1.0"'
        exit 1
    }
}

$Archive = "seite-$Target.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$Archive"
$ChecksumsUrl = "https://github.com/$Repo/releases/download/$Version/checksums-sha256.txt"

Write-Info "Installing seite $Version for $Target"

# --- Download ---
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "page-install-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    Write-Info "Downloading $Archive..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile (Join-Path $TmpDir $Archive) -UseBasicParsing
    Invoke-WebRequest -Uri $ChecksumsUrl -OutFile (Join-Path $TmpDir "checksums-sha256.txt") -UseBasicParsing

    # --- Verify checksum ---
    $ActualHash = (Get-FileHash (Join-Path $TmpDir $Archive) -Algorithm SHA256).Hash.ToLower()
    $ChecksumLine = Get-Content (Join-Path $TmpDir "checksums-sha256.txt") | Where-Object { $_ -match $Archive }
    if ($ChecksumLine) {
        $ExpectedHash = ($ChecksumLine -split '\s+')[0].ToLower()
        if ($ActualHash -ne $ExpectedHash) {
            Write-Err "Checksum verification failed!"
            Write-Host "  Expected: $ExpectedHash"
            Write-Host "  Actual:   $ActualHash"
            exit 1
        }
        Write-Info "Checksum verified"
    } else {
        Write-Warn "Could not find checksum for $Archive — skipping verification"
    }

    # --- Extract and install ---
    Expand-Archive -Path (Join-Path $TmpDir $Archive) -DestinationPath $TmpDir -Force

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Copy-Item (Join-Path $TmpDir $Binary) (Join-Path $InstallDir $Binary) -Force
    Write-Info "Installed seite to $(Join-Path $InstallDir $Binary)"

    # --- Check PATH ---
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Warn "$InstallDir is not in your PATH"
        Write-Host ""
        Write-Host "  Add it by running:"
        Write-Host ""
        Write-Host "    [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`$env:Path`", 'User')"
        Write-Host ""
        Write-Host "  Then restart your terminal."
        Write-Host ""
    }

    # --- Verify ---
    $PageExe = Join-Path $InstallDir $Binary
    if (Test-Path $PageExe) {
        $InstalledVersion = & $PageExe --version 2>$null
        if ($InstalledVersion) {
            Write-Info "Done! $InstalledVersion"
        } else {
            Write-Info "Done! Run 'page --version' to verify."
        }
    }
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
