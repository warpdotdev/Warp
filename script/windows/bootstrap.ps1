#!/usr/bin/env powershell

$ErrorActionPreference = 'Stop'

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

if (-not (Get-Command -Name cargo -Type Application -ErrorAction SilentlyContinue)) {
    Write-Output 'Installing rust...'
    Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile "$env:Temp\rustup-init.exe"
    & "$env:Temp\rustup-init.exe"
    Write-Output 'Please start a new terminal session so that cargo is in your PATH'
    exit 1
}

# Node.js and yarn are required at `cargo build` time by the
# `command-signatures-v2` crate. We deliberately do not auto-install Node or
# auto-run `corepack enable` — system-installed Node may need admin rights for
# corepack, and version-manager users (e.g. Volta) often manage yarn
# themselves. See the "Node.js setup" section of WARP.md for guidance.
$nodeMissing = -not (Get-Command -Name node -Type Application -ErrorAction SilentlyContinue)
$yarnMissing = -not (Get-Command -Name yarn -Type Application -ErrorAction SilentlyContinue)
if ($nodeMissing -or $yarnMissing) {
    Write-Error 'Missing Node.js and/or yarn (both required at `cargo build` time).'
    Write-Error 'See the "Node.js setup" section of WARP.md for installation guidance.'
    exit 1
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
