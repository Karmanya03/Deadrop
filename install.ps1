# deadrop installer for Windows — downloads the binary, renames it, adds to PATH
# Usage: irm https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$repo = "Karmanya03/Deadrop"
$binary = "ded"
$asset = "ded-windows-x86_64.exe"
$installDir = "$env:USERPROFILE\.local\bin"

Write-Host ""
Write-Host "     ██████╗ ███████╗ █████╗ ██████╗ ██████╗  ██████╗ ██████╗ " -ForegroundColor Cyan
Write-Host "     ██╔══██╗██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗██╔══██╗" -ForegroundColor Cyan
Write-Host "     ██║  ██║█████╗  ███████║██║  ██║██████╔╝██║   ██║██████╔╝" -ForegroundColor Cyan
Write-Host "     ██║  ██║██╔══╝  ██╔══██║██║  ██║██╔══██╗██║   ██║██╔═══╝ " -ForegroundColor Cyan
Write-Host "     ██████╔╝███████╗██║  ██║██████╔╝██║  ██║╚██████╔╝██║     " -ForegroundColor Cyan
Write-Host "     ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝     " -ForegroundColor Cyan
Write-Host ""
Write-Host "  Zero-knowledge encrypted file sharing" -ForegroundColor White
Write-Host ""

# --- Download ---
$downloadUrl = "https://github.com/$repo/releases/latest/download/$asset"
$destPath = Join-Path $installDir "$binary.exe"

Write-Host "  Platform:      windows" -ForegroundColor White
Write-Host "  Architecture:  x86_64" -ForegroundColor White
Write-Host "  Binary:        $asset" -ForegroundColor White
Write-Host "  Install to:    $installDir" -ForegroundColor White
Write-Host ""

Write-Host "  Downloading $asset..." -ForegroundColor Yellow

# Create install directory
if (!(Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $destPath -UseBasicParsing
} catch {
    Write-Host "  Download failed. Release may not exist yet." -ForegroundColor Red
    Write-Host "  Check: https://github.com/$repo/releases" -ForegroundColor Red
    exit 1
}

if (!(Test-Path $destPath) -or (Get-Item $destPath).Length -eq 0) {
    Write-Host "  Download failed — file is empty or missing." -ForegroundColor Red
    exit 1
}

Write-Host "  Installed to $destPath" -ForegroundColor Green

# --- Add to PATH ---
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$installDir", "User")
    $env:Path = "$env:Path;$installDir"
    Write-Host "  Added $installDir to user PATH" -ForegroundColor Green
    Write-Host "  Restart your terminal for PATH changes to take effect" -ForegroundColor Yellow
} else {
    Write-Host "  $installDir already in PATH" -ForegroundColor Cyan
}

# --- Verify ---
Write-Host ""
Write-Host "  deadrop is ready!" -ForegroundColor Green
Write-Host ""
Write-Host "  Usage:  ded ./secret-file.pdf" -ForegroundColor White
Write-Host "  Help:   ded --help" -ForegroundColor White
Write-Host ""
Write-Host "  Drop files. Leave no trace." -ForegroundColor Cyan
Write-Host ""
