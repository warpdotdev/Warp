#!/usr/bin/env powershell

[CmdletBinding()]
param(
    [ValidateSet('dev', 'preview', 'stable', 'oss')]
    [string]$Channel = 'oss',

    [string]$AssetPath,

    [string]$Tag = '',
    [string]$BaseRef = '',
    [string]$ToRef = 'HEAD',
    [string]$Repo = '',
    [string]$AuthorPattern = 'cesar|cesaryuan|cesaryuan@qq\.com',
    [string]$NotesPath = '',

    [switch]$Draft = $false,
    [switch]$Prerelease = $false,
    [switch]$DryRun = $false
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Assert-CommandExists {
    param([Parameter(Mandatory = $true)][string]$Name)

    if (-not (Get-Command -Name $Name -Type Application -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Invoke-ExternalCapture {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $output = & $FilePath @Arguments 2>&1
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        $rendered = ($output | ForEach-Object { "$_" }) -join "`n"
        throw "Command failed: $FilePath $($Arguments -join ' ')`n$rendered"
    }

    return (($output | ForEach-Object { "$_" }) -join "`n").Trim()
}

function Invoke-External {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed: $FilePath $($Arguments -join ' ')"
    }
}

function Test-LocalTagExists {
    param([Parameter(Mandatory = $true)][string]$ReleaseTag)

    & git rev-parse --verify --quiet "refs/tags/$ReleaseTag" *> $null
    return $LASTEXITCODE -eq 0
}

function Resolve-GitHubRepo {
    param([string]$ExplicitRepo)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitRepo)) {
        return $ExplicitRepo
    }

    $originUrl = Invoke-ExternalCapture -FilePath 'git' -Arguments @('remote', 'get-url', 'origin')

    if ($originUrl -match '^git@github\.com:(?<repo>.+?)(\.git)?$') {
        return $matches.repo
    }

    if ($originUrl -match '^https://github\.com/(?<repo>.+?)(\.git)?$') {
        return $matches.repo
    }

    throw "Could not infer GitHub repo from origin URL: $originUrl"
}

function Get-ChannelConfig {
    param([Parameter(Mandatory = $true)][string]$ReleaseChannel)

    $configPath = Join-Path $RepoRoot '.github/workflows/release_configurations.json'
    if (-not (Test-Path -LiteralPath $configPath)) {
        return $null
    }

    $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    return $config.channels | Where-Object { $_.channel -eq $ReleaseChannel } | Select-Object -First 1
}

function Get-CommitSha {
    param([Parameter(Mandatory = $true)][string]$Ref)

    return (Invoke-ExternalCapture -FilePath 'git' -Arguments @('rev-parse', "$Ref^{commit}")).Trim()
}

function New-ReleaseTag {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseChannel,
        [string]$GitHubRepo = '',
        [switch]$SkipRemoteCheck = $false
    )

    $dateFormatted = Get-Date -Format 'yyyy.MM.dd.HH.mm'
    $baseTag = "v0.$dateFormatted.$ReleaseChannel"
    $suffix = 0

    while ($true) {
        $candidate = '{0}_{1:d2}' -f $baseTag, $suffix
        $existsLocally = Test-LocalTagExists -ReleaseTag $candidate
        $existsRemotely = $false
        if (-not $SkipRemoteCheck -and -not [string]::IsNullOrWhiteSpace($GitHubRepo)) {
            $existsRemotely = Test-RemoteTagExists -ReleaseTag $candidate -GitHubRepo $GitHubRepo
        }

        if (-not $existsLocally -and -not $existsRemotely) {
            return $candidate
        }

        $suffix += 1
    }
}

function Resolve-BaseRef {
    param(
        [string]$ExplicitBaseRef,
        [string]$TargetTag,
        [string]$TargetRef,
        [string]$TargetCommitSha
    )

    if (-not [string]::IsNullOrWhiteSpace($ExplicitBaseRef)) {
        return $ExplicitBaseRef
    }

    $tagOutput = Invoke-ExternalCapture -FilePath 'git' -Arguments @(
        'for-each-ref',
        'refs/tags',
        '--merged',
        $TargetRef,
        '--sort=-creatordate',
        '--format=%(refname:short)'
    )

    $tags = @()
    if (-not [string]::IsNullOrWhiteSpace($tagOutput)) {
        $tags = $tagOutput -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    }

    foreach ($candidate in $tags) {
        if ($candidate -eq $TargetTag) {
            continue
        }

        $candidateCommitSha = Get-CommitSha -Ref $candidate
        if (-not [string]::IsNullOrWhiteSpace($TargetCommitSha) -and $candidateCommitSha -eq $TargetCommitSha) {
            continue
        }

        if (-not [string]::IsNullOrWhiteSpace($candidate)) {
            return $candidate
        }
    }

    return ''
}

function Get-CommitSubjects {
    param(
        [string]$StartRef,
        [Parameter(Mandatory = $true)][string]$EndRef,
        [Parameter(Mandatory = $true)][string]$CommitAuthorPattern
    )

    $arguments = @('log', '--no-merges', '--format=%an%x09%ae%x09%s')
    if (-not [string]::IsNullOrWhiteSpace($StartRef)) {
        $arguments += "$StartRef..$EndRef"
    } else {
        $arguments += $EndRef
    }

    $output = Invoke-ExternalCapture -FilePath 'git' -Arguments $arguments
    if ([string]::IsNullOrWhiteSpace($output)) {
        return @()
    }

    $subjects = New-Object System.Collections.Generic.List[string]
    foreach ($line in ($output -split "`r?`n")) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }

        $parts = $line -split "`t", 3
        if ($parts.Count -lt 3) {
            continue
        }

        $authorName = $parts[0]
        $authorEmail = $parts[1]
        $subject = $parts[2]
        if ($subject -match '^Merge\b') {
            continue
        }

        if ($authorName -match $CommitAuthorPattern -or $authorEmail -match $CommitAuthorPattern) {
            $subjects.Add($subject)
        }
    }

    return $subjects
}

function Build-ReleaseNotes {
    param(
        [string[]]$Subjects,
        [string]$StartRef,
        [string]$EndRef,
        [string]$GitHubRepo,
        [string]$TargetCommitSha
    )

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('# Release Notes')

    if (-not [string]::IsNullOrWhiteSpace($StartRef)) {
        $shortSha = if ($TargetCommitSha.Length -ge 8) { $TargetCommitSha.Substring(0, 8) } else { $TargetCommitSha }
        $compareLabel = "$StartRef...$shortSha"
        $compareUrl = "https://github.com/$GitHubRepo/compare/$StartRef...$TargetCommitSha"
        $lines.Add("Commit range: [$compareLabel]($compareUrl)")
    } else {
        $shortSha = if ($TargetCommitSha.Length -ge 8) { $TargetCommitSha.Substring(0, 8) } else { $TargetCommitSha }
        $commitUrl = "https://github.com/$GitHubRepo/commit/$TargetCommitSha"
        $lines.Add("Commit: [$shortSha]($commitUrl)")
    }

    $lines.Add('')

    if ($Subjects.Count -eq 0) {
        $lines.Add('No matching commit messages found.')
    } else {
        foreach ($subject in $Subjects) {
            $lines.Add("- $subject")
        }
    }

    return ($lines -join "`n").Trim() + "`n"
}

function Get-NotesFilePath {
    param(
        [string]$ExplicitPath,
        [Parameter(Mandatory = $true)][string]$ReleaseTag
    )

    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        $directory = Split-Path -Parent $ExplicitPath
        if (-not [string]::IsNullOrWhiteSpace($directory)) {
            New-Item -ItemType Directory -Force -Path $directory | Out-Null
        }
        return (Resolve-Path -LiteralPath (New-Item -ItemType File -Force -Path $ExplicitPath).FullName).Path
    }

    $notesDirectory = Join-Path $RepoRoot 'target/release-notes'
    New-Item -ItemType Directory -Force -Path $notesDirectory | Out-Null
    return (Join-Path $notesDirectory "$ReleaseTag.md")
}

function Test-ReleaseExists {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseTag,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    & gh release view $ReleaseTag --repo $GitHubRepo --json tagName *> $null
    return $LASTEXITCODE -eq 0
}

function Test-RemoteCommitExists {
    param(
        [Parameter(Mandatory = $true)][string]$CommitSha,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    & gh api "repos/$GitHubRepo/commits/$CommitSha" *> $null
    return $LASTEXITCODE -eq 0
}

function Test-RemoteTagExists {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseTag,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    & gh api "repos/$GitHubRepo/git/ref/tags/$ReleaseTag" *> $null
    return $LASTEXITCODE -eq 0
}

function Get-RemoteTagCommitSha {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseTag,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    if (-not (Test-RemoteTagExists -ReleaseTag $ReleaseTag -GitHubRepo $GitHubRepo)) {
        return ''
    }

    $type = Invoke-ExternalCapture -FilePath 'gh' -Arguments @(
        'api', "repos/$GitHubRepo/git/ref/tags/$ReleaseTag", '--jq', '.object.type'
    )
    $sha = Invoke-ExternalCapture -FilePath 'gh' -Arguments @(
        'api', "repos/$GitHubRepo/git/ref/tags/$ReleaseTag", '--jq', '.object.sha'
    )

    if ($type -eq 'commit') {
        return $sha.Trim()
    }

    if ($type -eq 'tag') {
        return (Invoke-ExternalCapture -FilePath 'gh' -Arguments @(
            'api', "repos/$GitHubRepo/git/tags/$sha", '--jq', '.object.sha'
        )).Trim()
    }

    return ''
}

function Find-ExistingReleaseTagForCommit {
    param(
        [Parameter(Mandatory = $true)][string]$CommitSha,
        [Parameter(Mandatory = $true)][string]$ReleaseChannel,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    $releaseTagOutput = Invoke-ExternalCapture -FilePath 'gh' -Arguments @(
        'release', 'list', '--repo', $GitHubRepo, '--limit', '100', '--json', 'tagName', '--jq', '.[].tagName'
    )
    if ([string]::IsNullOrWhiteSpace($releaseTagOutput)) {
        return ''
    }

    $channelPattern = '\.{0}_[0-9][0-9]$' -f [System.Text.RegularExpressions.Regex]::Escape($ReleaseChannel)
    foreach ($candidate in ($releaseTagOutput -split "`r?`n")) {
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            continue
        }

        if ($candidate -notmatch $channelPattern) {
            continue
        }

        $candidateCommitSha = Get-RemoteTagCommitSha -ReleaseTag $candidate -GitHubRepo $GitHubRepo
        if ($candidateCommitSha -eq $CommitSha) {
            return $candidate
        }
    }

    return ''
}

function Get-ReleaseBody {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseTag,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    return Invoke-ExternalCapture -FilePath 'gh' -Arguments @(
        'release', 'view', $ReleaseTag, '--repo', $GitHubRepo, '--json', 'body', '--jq', '.body // ""'
    )
}

function Cleanup-CreatedTag {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseTag,
        [Parameter(Mandatory = $true)][string]$GitHubRepo
    )

    if (Test-ReleaseExists -ReleaseTag $ReleaseTag -GitHubRepo $GitHubRepo) {
        Invoke-External -FilePath 'gh' -Arguments @(
            'release', 'delete', $ReleaseTag, '--repo', $GitHubRepo, '--yes', '--cleanup-tag'
        )
        return
    }

    if (Test-RemoteTagExists -ReleaseTag $ReleaseTag -GitHubRepo $GitHubRepo) {
        Invoke-External -FilePath 'gh' -Arguments @(
            'api', '--method', 'DELETE', "repos/$GitHubRepo/git/refs/tags/$ReleaseTag"
        )
    }
}

function Test-ShouldPreserveExistingNotes {
    param(
        [string]$ExistingBody,
        [string]$GeneratedBody,
        [string]$DefaultBody
    )

    if ([string]::IsNullOrWhiteSpace($ExistingBody)) {
        return $false
    }

    $normalizedExisting = $ExistingBody.Trim()
    $normalizedGenerated = if ($null -eq $GeneratedBody) { '' } else { $GeneratedBody.Trim() }
    $normalizedDefault = if ($null -eq $DefaultBody) { '' } else { $DefaultBody.Trim() }

    if ($normalizedExisting -eq $normalizedGenerated) {
        return $false
    }

    if (-not [string]::IsNullOrWhiteSpace($normalizedDefault) -and $normalizedExisting -eq $normalizedDefault) {
        return $false
    }

    return $true
}

Assert-CommandExists -Name 'git'

$hasAssetPath = -not [string]::IsNullOrWhiteSpace($AssetPath)
$resolvedAssetPath = ''
if ($hasAssetPath) {
    $resolvedAssetPath = [System.IO.Path]::GetFullPath($AssetPath)
    if (-not (Test-Path -LiteralPath $resolvedAssetPath -PathType Leaf)) {
        throw "Asset not found: $resolvedAssetPath"
    }
}

$gitHubRepo = Resolve-GitHubRepo -ExplicitRepo $Repo
$channelConfig = Get-ChannelConfig -ReleaseChannel $Channel
$releaseBaseName = if ($null -ne $channelConfig) { $channelConfig.release_base_name } else { "$Channel Release" }
$defaultReleaseBody = if ($null -ne $channelConfig) { $channelConfig.release_body_text } else { '' }

$reusedExistingRelease = $false
$targetCommitSha = Get-CommitSha -Ref $ToRef

if ([string]::IsNullOrWhiteSpace($Tag)) {
    $Tag = New-ReleaseTag -ReleaseChannel $Channel -SkipRemoteCheck
}

$effectiveBaseRef = Resolve-BaseRef -ExplicitBaseRef $BaseRef -TargetTag $Tag -TargetRef $ToRef -TargetCommitSha $targetCommitSha
$commitSubjects = Get-CommitSubjects -StartRef $effectiveBaseRef -EndRef $ToRef -CommitAuthorPattern $AuthorPattern
$notesBody = Build-ReleaseNotes -Subjects $commitSubjects -StartRef $effectiveBaseRef -EndRef $ToRef -GitHubRepo $gitHubRepo -TargetCommitSha $targetCommitSha
$resolvedNotesPath = Get-NotesFilePath -ExplicitPath $NotesPath -ReleaseTag $Tag
Set-Content -LiteralPath $resolvedNotesPath -Value $notesBody -Encoding utf8

$title = "$releaseBaseName $Tag"

Write-Output "Repo: $gitHubRepo"
Write-Output "Tag: $Tag"
Write-Output "Title: $title"
Write-Output "Asset: $(if ($hasAssetPath) { $resolvedAssetPath } else { '(none)' })"
Write-Output "BaseRef: $effectiveBaseRef"
Write-Output "ToRef: $ToRef"
Write-Output "TargetCommit: $targetCommitSha"
Write-Output "ReusedExistingRelease: $reusedExistingRelease"
Write-Output "Notes: $resolvedNotesPath"

if ($DryRun) {
    Write-Output ''
    Write-Output '--- Release Notes Preview ---'
    Write-Output $notesBody
    exit 0
}

Assert-CommandExists -Name 'gh'

Invoke-External -FilePath 'gh' -Arguments @('auth', 'status')

$existingReleaseTag = Find-ExistingReleaseTagForCommit -CommitSha $targetCommitSha -ReleaseChannel $Channel -GitHubRepo $gitHubRepo
if (-not [string]::IsNullOrWhiteSpace($existingReleaseTag) -and $existingReleaseTag -ne $Tag) {
    $Tag = $existingReleaseTag
    $reusedExistingRelease = $true
    $effectiveBaseRef = Resolve-BaseRef -ExplicitBaseRef $BaseRef -TargetTag $Tag -TargetRef $ToRef -TargetCommitSha $targetCommitSha
    $commitSubjects = Get-CommitSubjects -StartRef $effectiveBaseRef -EndRef $ToRef -CommitAuthorPattern $AuthorPattern
    $notesBody = Build-ReleaseNotes -Subjects $commitSubjects -StartRef $effectiveBaseRef -EndRef $ToRef -GitHubRepo $gitHubRepo -TargetCommitSha $targetCommitSha
    $resolvedNotesPath = Get-NotesFilePath -ExplicitPath $NotesPath -ReleaseTag $Tag
    Set-Content -LiteralPath $resolvedNotesPath -Value $notesBody -Encoding utf8
    $title = "$releaseBaseName $Tag"
} elseif (-not $reusedExistingRelease -and -not (Test-ReleaseExists -ReleaseTag $Tag -GitHubRepo $gitHubRepo) -and -not $PSBoundParameters.ContainsKey('Tag')) {
    $Tag = New-ReleaseTag -ReleaseChannel $Channel -GitHubRepo $gitHubRepo
    $effectiveBaseRef = Resolve-BaseRef -ExplicitBaseRef $BaseRef -TargetTag $Tag -TargetRef $ToRef -TargetCommitSha $targetCommitSha
    $commitSubjects = Get-CommitSubjects -StartRef $effectiveBaseRef -EndRef $ToRef -CommitAuthorPattern $AuthorPattern
    $notesBody = Build-ReleaseNotes -Subjects $commitSubjects -StartRef $effectiveBaseRef -EndRef $ToRef -GitHubRepo $gitHubRepo -TargetCommitSha $targetCommitSha
    $resolvedNotesPath = Get-NotesFilePath -ExplicitPath $NotesPath -ReleaseTag $Tag
    Set-Content -LiteralPath $resolvedNotesPath -Value $notesBody -Encoding utf8
    $title = "$releaseBaseName $Tag"
}

Write-Output "Using release tag: $Tag"
Write-Output "Reused existing release: $reusedExistingRelease"

if (-not $reusedExistingRelease -and -not (Test-RemoteCommitExists -CommitSha $targetCommitSha -GitHubRepo $gitHubRepo)) {
    throw "Commit $targetCommitSha is not available on github.com/$gitHubRepo yet. Push the commit first, then rerun this script."
}

try {
    $releaseExistedBefore = Test-ReleaseExists -ReleaseTag $Tag -GitHubRepo $gitHubRepo
    $shouldCleanupCreatedTagOnFailure = (-not $releaseExistedBefore) -and (-not $PSBoundParameters.ContainsKey('Tag'))

    if ($releaseExistedBefore) {
        $existingBody = Get-ReleaseBody -ReleaseTag $Tag -GitHubRepo $gitHubRepo
        $existingReleasePointsToTarget = (Get-RemoteTagCommitSha -ReleaseTag $Tag -GitHubRepo $gitHubRepo) -eq $targetCommitSha
        $preserveExistingNotes = Test-ShouldPreserveExistingNotes -ExistingBody $existingBody -GeneratedBody $notesBody -DefaultBody $defaultReleaseBody

        if (-not $existingReleasePointsToTarget -or -not $preserveExistingNotes) {
            $editArguments = @(
                'release', 'edit', $Tag,
                '--repo', $gitHubRepo,
                '--title', $title
            )

            if (-not $preserveExistingNotes) {
                $editArguments += @('--notes-file', $resolvedNotesPath)
            }

            if ($Draft) {
                $editArguments += '--draft'
            }
            if ($Prerelease) {
                $editArguments += '--prerelease'
            }
            Invoke-External -FilePath 'gh' -Arguments $editArguments
        }

        if ($hasAssetPath) {
            Invoke-External -FilePath 'gh' -Arguments @('release', 'upload', $Tag, $resolvedAssetPath, '--repo', $gitHubRepo, '--clobber')
        }
    } else {
        $createArguments = @(
            'release', 'create', $Tag,
            '--repo', $gitHubRepo,
            '--title', $title,
            '--notes-file', $resolvedNotesPath,
            '--target', $targetCommitSha
        )
        if ($hasAssetPath) {
            $createArguments += $resolvedAssetPath
        }
        if ($Draft) {
            $createArguments += '--draft'
        }
        if ($Prerelease) {
            $createArguments += '--prerelease'
        }
        Invoke-External -FilePath 'gh' -Arguments $createArguments
    }
} catch {
    if ($shouldCleanupCreatedTagOnFailure) {
        Write-Warning "Release publishing failed. Cleaning up newly created tag/release '$Tag'."
        Cleanup-CreatedTag -ReleaseTag $Tag -GitHubRepo $gitHubRepo
    }

    throw
}

Write-Output ''
Write-Output "GitHub release ready: $title"
