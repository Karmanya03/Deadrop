# deadrop installer for Windows — downloads the binary, renames it, adds to PATH
# Usage: irm https://raw.githubusercontent.com/Karmanya03/Deadrop/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

function Invoke-DeadropInstall {
    $repo = "Karmanya03/Deadrop"
    $binary = "ded"
    $asset = "ded-windows-x86_64.exe"
    $installDir = Join-Path $env:USERPROFILE ".local\bin"
    $downloadUrl = "https://github.com/$repo/releases/latest/download/$asset"
    $destPath = Join-Path $installDir "$binary.exe"

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

    Write-Host "  Platform:      windows" -ForegroundColor White
    Write-Host "  Architecture:  x86_64" -ForegroundColor White
    Write-Host "  Binary:        $asset" -ForegroundColor White
    Write-Host "  Install to:    $installDir" -ForegroundColor White
    Write-Host ""
    Write-Host "  Downloading $asset..." -ForegroundColor Yellow

    if (!(Test-Path $installDir)) {
        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    }

    # Use a temp file first; move into place only after successful download.
    $tmpPath = Join-Path $env:TEMP ("deadrop-install-" + [Guid]::NewGuid().ToString() + ".exe")

    # Keep compatible across Windows PowerShell 5.1 and PowerShell 7+.
    $iwrParams = @{ Uri = $downloadUrl; OutFile = $tmpPath }
    if ((Get-Command Invoke-WebRequest).Parameters.ContainsKey("UseBasicParsing")) {
        $iwrParams["UseBasicParsing"] = $true
    }

    try {
        Invoke-WebRequest @iwrParams
    } catch {
        if (Test-Path $tmpPath) { Remove-Item -Force $tmpPath -ErrorAction SilentlyContinue }
        throw "Download failed. Release may not exist yet. Check: https://github.com/$repo/releases`n$($_.Exception.Message)"
    }

    if (!(Test-Path $tmpPath) -or (Get-Item $tmpPath).Length -eq 0) {
        if (Test-Path $tmpPath) { Remove-Item -Force $tmpPath -ErrorAction SilentlyContinue }
        throw "Download failed: file is empty or missing."
    }

    Move-Item -Force $tmpPath $destPath
    Write-Host "  Installed to $destPath" -ForegroundColor Green

    # --- Add to PATH ---
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ([string]::IsNullOrWhiteSpace($currentPath)) {
        [Environment]::SetEnvironmentVariable("Path", $installDir, "User")
        $env:Path = "$env:Path;$installDir"
        Write-Host "  Added $installDir to user PATH" -ForegroundColor Green
        Write-Host "  Restart your terminal for PATH changes to take effect" -ForegroundColor Yellow
    } elseif ($currentPath -notlike "*$installDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$installDir", "User")
        $env:Path = "$env:Path;$installDir"
        Write-Host "  Added $installDir to user PATH" -ForegroundColor Green
        Write-Host "  Restart your terminal for PATH changes to take effect" -ForegroundColor Yellow
    } else {
        Write-Host "  $installDir already in PATH" -ForegroundColor Cyan
    }

    Write-Host ""
    Write-Host "  deadrop is ready!" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Usage:  ded ./secret-file.pdf" -ForegroundColor White
    Write-Host "  Help:   ded --help" -ForegroundColor White
    Write-Host ""
    Write-Host "  Drop files. Leave no trace." -ForegroundColor Cyan
    Write-Host ""
}

try {
    Invoke-DeadropInstall
} catch {
    Write-Host "" -ForegroundColor Red
    Write-Host "  Install failed:" -ForegroundColor Red
    Write-Host "  $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "" -ForegroundColor Red
    return
}
