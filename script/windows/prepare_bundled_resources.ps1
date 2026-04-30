#
# Prepares bundled resources for distribution on Windows.
#
# This script copies resources that should be bundled with Warp into a
# destination directory. It is used by the Windows build script.
#
# Usage:
#   prepare_bundled_resources.ps1 -DestinationDir <destination_directory> [-Channel <channel>] [-CargoProfile <profile>] [-CargoFeatures <features>] [-CargoTarget <target>]
#
# Arguments:
#   DestinationDir:  The directory where resources should be installed.
#                    Resources will be copied to subdirectories within this
#                    path (e.g., $DEST_DIR\skills).
#   Channel:         (Optional) Release channel. Used to include
#                    channel-gated skills.
#   CargoProfile:    (Optional) Cargo build profile to use when generating
#                    the settings schema.
#   CargoFeatures:   (Optional) Comma-separated cargo features to enable when
#                    generating the settings schema. Should match the
#                    features used to build the main binary so that cargo
#                    can reuse the existing compilation artifacts.  Also
#                    ensures that feature-gated settings are included in the
#                    generated schema.
#   CargoTarget:     (Optional) Rust target triple to pass via --target.
#                    Should match the target used to build the main binary.
#                    NOTE: only set this when the target can be executed on
#                    the build host. `cargo run` will attempt to execute
#                    the resulting binary, so this must be omitted when
#                    cross-compiling to a target the host can't run
#                    (e.g. aarch64-pc-windows-msvc on an x64 runner).

Param(
    [Parameter(Mandatory = $true)]
    [String]$DestinationDir,

    [Parameter(Mandatory = $false)]
    [String]$Channel = '',

    [Parameter(Mandatory = $false)]
    [String]$CargoProfile = '',

    [Parameter(Mandatory = $false)]
    [String]$CargoFeatures = '',

    [Parameter(Mandatory = $false)]
    [String]$CargoTarget = ''
)

$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = (Get-Item "$ScriptDir\..\.." | Select-Object -ExpandProperty FullName)
$ResourcesSource = Join-Path $RepoRoot 'resources'

# Validate that the source resources directory exists
if (-Not (Test-Path $ResourcesSource -PathType Container)) {
    Write-Error "Resources directory not found at $ResourcesSource"
    exit 1
}

# Create the destination directory if it doesn't exist
if (-Not (Test-Path $DestinationDir)) {
    New-Item -ItemType Directory -Path $DestinationDir -Force | Out-Null
}

# Copy bundled resources
$BundledSource = Join-Path $ResourcesSource 'bundled'
if (Test-Path $BundledSource -PathType Container) {
    $BundledDestination = Join-Path $DestinationDir 'bundled'
    Write-Output "Copying bundled resources to $BundledDestination"
    if (Test-Path $BundledDestination -PathType Container) {
        Remove-Item -Path $BundledDestination -Recurse -Force
    }
    Copy-Item -Path $BundledSource -Destination $BundledDestination -Recurse -Force
} else {
    Write-Warning "No bundled directory found at $BundledSource"
}

if ($env:GIT_RELEASE_TAG) {
    $VersionMetadataDir = Join-Path (Join-Path $DestinationDir 'bundled') 'metadata'
    $VersionMetadataPath = Join-Path $VersionMetadataDir 'version.json'
    Write-Output "Writing bundled Warp version metadata to $VersionMetadataPath"
    if (-Not (Test-Path $VersionMetadataDir -PathType Container)) {
        New-Item -ItemType Directory -Path $VersionMetadataDir -Force | Out-Null
    }

    @{ warp_version = $env:GIT_RELEASE_TAG } |
        ConvertTo-Json |
        Set-Content -Path $VersionMetadataPath -Encoding utf8
}

# Copy channel-gated skills matching the current release channel.
$GatedSource = Join-Path (Join-Path $RepoRoot 'resources') 'channel-gated-skills'
$DestSkills = Join-Path (Join-Path $DestinationDir 'bundled') 'skills'

if ($Channel -and (Test-Path $GatedSource -PathType Container)) {
    Write-Output "Copying channel-gated skills for channel '$Channel'..."

    # Error out if a stable/ gate directory exists.
    $StableDir = Join-Path $GatedSource 'stable'
    if (Test-Path $StableDir -PathType Container) {
        Write-Error "Found a 'stable/' directory in $GatedSource. The stable channel does not use gated skills. Move stable-ready skills to resources/skills/ instead."
        exit 1
    }

    # Gate labels ordered from most-inclusive to least-inclusive.
    $GateOrder = @('dogfood', 'preview')

    # Map the release channel to its gate label.
    switch ($Channel) {
        'local' { $Gate = 'dogfood' }
        'dev' { $Gate = 'dogfood' }
        'preview' { $Gate = 'preview' }
        default {
            Write-Output "  Channel '$Channel' has no gated skills, skipping"
            $Gate = $null
        }
    }

    if ($Gate) {
        # Build the set of included gates (progressive: this gate and all after it).
        $GateIndex = [array]::IndexOf($GateOrder, $Gate)
        $IncludedGates = $GateOrder[$GateIndex..($GateOrder.Length - 1)]

        foreach ($GateDir in Get-ChildItem -Path $GatedSource -Directory | Sort-Object Name) {
            if ($GateDir.Name -notin $IncludedGates) {
                $Skills = (Get-ChildItem -Path $GateDir.FullName -Directory | Sort-Object Name | ForEach-Object { $_.Name }) -join ', '
                Write-Output "  Skipping gate '$($GateDir.Name)' (channel '$Channel') - would include: $Skills"
                continue
            }

            foreach ($SkillDir in Get-ChildItem -Path $GateDir.FullName -Directory | Sort-Object Name) {
                $Dest = Join-Path $DestSkills $SkillDir.Name
                Write-Output "  Copying gated skill: $($SkillDir.Name) (gate: $($GateDir.Name))"
                Copy-Item -Path $SkillDir.FullName -Destination $Dest -Recurse -Force
            }
        }
    }
}

# Generate third-party license attribution.
#
# Additional (non-Cargo) third-party license files to include in the output.
# When adding a new third-party component to the bundle, add its license file
# to the repo alongside the component and add an entry here.
# Cross-platform components:
$AdditionalLicenses = @(
    @{ Name = 'Alacritty (alacritty_terminal)'; License = 'Apache-2.0'; Path = 'crates\warp_terminal\src\model\LICENSE-ALACRITTY' },
    @{ Name = 'Hack Font'; License = 'MIT'; Path = 'app\assets\bundled\fonts\hack\LICENSE.md' },
    @{ Name = 'Roboto Font'; License = 'SIL Open Font License'; Path = 'app\assets\bundled\fonts\roboto\LICENSE.txt' },
    @{ Name = 'bash-preexec'; License = 'MIT'; Path = 'app\assets\bundled\bootstrap\bash-preexec-LICENSE.md' },
    @{ Name = 'Claude API Skill'; License = 'Apache-2.0'; Path = 'resources\bundled\skills\claude-api\LICENSE.txt' },
    @{ Name = 'rudder-sdk-rust'; License = 'MIT'; Path = 'app\src\server\telemetry\LICENSE-RUDDER-SDK-RUST.txt' },
    @{ Name = 'Windows Terminal'; License = 'MIT'; Path = 'app\assets\windows\LICENSE-WINDOWS-TERMINAL' },
    @{ Name = 'GitHub Desktop'; License = 'MIT'; Path = 'app\src\code_review\GITHUB-DESKTOP-LICENSE' }
)
# Windows-only components:
$AdditionalLicenses += @(
    @{ Name = 'OpenConsole / ConPTY (Windows Terminal)'; License = 'MIT'; Path = 'app\assets\windows\LICENSE-WINDOWS-TERMINAL' },
    @{ Name = 'DirectX Shader Compiler'; License = 'NCSA'; Path = 'app\assets\windows\LICENSE-DXC' }
)

$LicensesOutput = Join-Path $DestinationDir 'THIRD_PARTY_LICENSES.txt'
Write-Output "Generating third-party licenses at $LicensesOutput"
cargo about generate --workspace --manifest-path "$RepoRoot\Cargo.toml" -c "$RepoRoot\about.toml" -o "$LicensesOutput" "$RepoRoot\about.hbs"
if (-Not $?) {
    Write-Error 'Failed to generate third-party licenses'
    exit 1
}

# Append additional (non-Cargo) third-party licenses.
foreach ($entry in $AdditionalLicenses) {
    $LicenseFile = Join-Path $RepoRoot $entry.Path
    if (-Not (Test-Path $LicenseFile)) {
        Write-Error "License file not found: $LicenseFile"
        exit 1
    }
    Add-Content -Path $LicensesOutput -Value ''
    Add-Content -Path $LicensesOutput -Value "$($entry.Name) ($($entry.License))"
    Add-Content -Path $LicensesOutput -Value ('-' * 80)
    Get-Content -Path $LicenseFile | Add-Content -Path $LicensesOutput
    Add-Content -Path $LicensesOutput -Value ''
}

# Generate settings JSON schema unless explicitly skipped.
if ($env:SKIP_SETTINGS_SCHEMA -ne '1') {
    $SchemaOutput = Join-Path $DestinationDir 'settings_schema.json'
    Write-Output "Generating settings schema at $SchemaOutput"

    # Pass the same package, profile, features, and target as the main
    # build so that cargo can reuse compilation artifacts (rather than
    # recompiling any dependencies whose enabled features differ).  Also
    # ensures that feature-gated settings are included in the generated
    # schema.
    $SchemaCmd = @('run', '--manifest-path', (Join-Path $RepoRoot 'Cargo.toml'), '-p', 'warp')
    if ($CargoProfile) {
        $SchemaCmd += @('--profile', $CargoProfile)
    }
    if ($CargoFeatures) {
        $SchemaCmd += @('--features', $CargoFeatures)
    }
    if ($CargoTarget) {
        $SchemaCmd += @('--target', $CargoTarget)
    }
    $SchemaCmd += @('--bin', 'generate_settings_schema', '--')
    if ($Channel) {
        $SchemaCmd += @('--channel', $Channel)
    }
    $SchemaCmd += $SchemaOutput

    & cargo @SchemaCmd
    if (-Not $?) {
        Write-Error 'Failed to generate settings schema'
        exit 1
    }
}

Write-Output "Successfully prepared bundled resources in $DestinationDir"
