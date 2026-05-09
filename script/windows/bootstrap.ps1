#!/usr/bin/env powershell
param(
    [switch]$Help,
    [switch]$InstallCommonSkills
)

$ErrorActionPreference = 'Stop'

function Show-Usage {
    Write-Output 'Usage: .\script\windows\bootstrap.ps1 [-Help] [-InstallCommonSkills]'
    Write-Output ''
    Write-Output 'Prepare this checkout for Warp development on Windows.'
    Write-Output ''
    Write-Output 'Options:'
    Write-Output '  -Help                 Show this help message.'
    Write-Output '  -InstallCommonSkills  Install or update common agent skills from skills-lock.json.'
    Write-Output ''
    Write-Output 'Environment:'
    Write-Output '  WARP_SKIP_COMMON_SKILLS_INSTALL=1'
    Write-Output '      Skip installing common agent skills.'
}

function Show-BootstrapPreview {
    Write-Output 'Warp bootstrap is starting for Windows.'
    Write-Output 'It will:'
    Write-Output '  - Check for Git for Windows.'
    Write-Output '  - Install Rust if cargo is unavailable.'
    Write-Output '  - Install Visual Studio Build Tools, jq, CMake, InnoSetup, and gcloud as needed.'
    Write-Output '  - Install Cargo test dependencies.'

    if (-not $InstallCommonSkills) {
        Write-Output '  - Skip common agent skills unless -InstallCommonSkills is provided.'
    } elseif ($env:WARP_SKIP_COMMON_SKILLS_INSTALL -eq '1') {
        Write-Output '  - Skip common agent skills because WARP_SKIP_COMMON_SKILLS_INSTALL=1.'
    } else {
        Write-Output '  - Install or update common agent skills from skills-lock.json if needed.'
    }

    Write-Output 'Run .\script\windows\bootstrap.ps1 -Help to see options and environment overrides.'
    Write-Output ''
}

if ($Help) {
    Show-Usage
    exit 0
}

Show-BootstrapPreview

# Git for Windows can be installed system-wide (Program Files) or per-user (LOCALAPPDATA\Programs\Git).
$gitBinCandidates = @(
    "$env:PROGRAMFILES\Git\bin",
    "$env:LOCALAPPDATA\Programs\Git\bin"
)
$gitBinDir = $gitBinCandidates | Where-Object { Test-Path -PathType Container $_ } | Select-Object -First 1
if (-not $gitBinDir) {
    Write-Error 'Git for Windows is required. Please install it at:'
    Write-Error 'https://gitforwindows.org/'
    exit 1
}
$env:PATH = "$gitBinDir;$env:PATH"

function Install-CommonSkill {
    & "$gitBinDir\bash.exe" "$PWD\script\install_common_skills" --if-needed
}

if (-not (Get-Command -Name cargo -Type Application -ErrorAction SilentlyContinue)) {
    Write-Output 'Installing rust...'
    Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile "$env:Temp\rustup-init.exe"
    & "$env:Temp\rustup-init.exe"
    Write-Output 'Please start a new terminal session so that cargo is in your PATH'
    exit 1
}

# Visual Studio Build Tools (MSVC compiler + linker + Windows SDK) are required to link Rust crates
# targeting x86_64-pc-windows-msvc.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$haveMsvcBuildTools = $false
if (Test-Path $vswhere) {
    $vsInstall = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 Microsoft.VisualStudio.Component.Windows11SDK.22621 `
        -property installationPath
    if ($vsInstall) { $haveMsvcBuildTools = $true }
}
if (-not $haveMsvcBuildTools) {
    Write-Output 'Installing Visual Studio Build Tools (MSVC + Windows SDK)...'
    winget install -e --id Microsoft.VisualStudio.2022.BuildTools `
        --accept-package-agreements --accept-source-agreements `
        --override '--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621 --includeRecommended'
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# A bash executable should come with Git for Windows
& "$gitBinDir\bash.exe" "$PWD\script\install_cargo_test_deps"

# Needed in wasm compilation for parsing the version of wasm-bindgen
winget install jqlang.jq

# CMake is needed to build some dependencies, e.g.: sentry-contrib-native.
winget install -e --id Kitware.CMake

# We use InnoSetup to build our release bundle installer.
winget install -e --id JRSoftware.InnoSetup

# If we don't see gcloud command, try adding the install location to the PATH.
if (-not (Get-Command -Name gcloud -Type Application -ErrorAction SilentlyContinue)) {
    $env:PATH += ";$env:LOCALAPPDATA\Google\Cloud SDK\google-cloud-sdk\bin"
}

# If we still don't see it, install it.
if (-not (Get-Command -Name gcloud -Type Application -ErrorAction SilentlyContinue)) {
    (New-Object Net.WebClient).DownloadFile('https://dl.google.com/dl/cloudsdk/channels/rapid/GoogleCloudSDKInstaller.exe', "$env:Temp\GoogleCloudSDKInstaller.exe")
    Start-Process "$env:Temp\GoogleCloudSDKInstaller.exe" -Wait
}

[string]$identityToken = gcloud auth print-identity-token
if ($identityToken.Trim().Length -eq 0) {
    Write-Output 'gcloud CLI authentication missing.  Press enter to continue...'
    Read-Host
    gcloud auth login
}

if ($InstallCommonSkills) {
    Install-CommonSkill
}
