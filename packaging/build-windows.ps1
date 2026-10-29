# Build dist\CampaignManager-<version>-windows-x64.zip on Windows.
#
# Needs (one-time):
#   1. Rust from https://rustup.rs (the default "x86_64-pc-windows-msvc" toolchain)
#   2. Visual Studio Build Tools with the "Desktop development with C++" workload
#      (rustup's installer offers to install these for you)
#
# Run from PowerShell in the source folder:
#   powershell -ExecutionPolicy Bypass -File packaging\build-windows.ps1
#
# The result is a portable app: unzip anywhere and run "Campaign Manager.exe". It needs no
# installer and nothing else installed (OpenGL 2.0+, which every Windows 10/11 PC has).

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Building Campaign Manager $version for Windows..."

cargo build --release -p campaign-manager
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$stage = "target\windows\Campaign Manager"
if (Test-Path "target\windows") { Remove-Item -Recurse -Force "target\windows" }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item "target\release\campaign-manager.exe" "$stage\Campaign Manager.exe"
Copy-Item README.md, LICENSE $stage
Copy-Item docs\statblock-import-spec.md $stage
Copy-Item docs\item-import-spec.md $stage

New-Item -ItemType Directory -Force -Path dist | Out-Null
$zip = "dist\CampaignManager-$version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Built $zip"
