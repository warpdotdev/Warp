$ErrorActionPreference = 'Stop'

$missing = $false

$node = Get-Command -Name node -Type Application -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Output 'Node.js is required to build Warp''s command-signatures package. Install Node.js 18 or newer, then rerun .\script\bootstrap.ps1.'
    $missing = $true
} else {
    $nodeVersion = (& node --version).Trim().TrimStart('v')
    $nodeMajor = 0
    if (-not [int]::TryParse(($nodeVersion -split '\.')[0], [ref]$nodeMajor) -or $nodeMajor -lt 18) {
        Write-Output "Node.js 18 or newer is required; found $(& node --version)."
        $missing = $true
    }
}

$yarn = Get-Command -Name yarn -Type Application -ErrorAction SilentlyContinue
if (-not $yarn) {
    Write-Output 'Yarn is required to build Warp''s command-signatures package.'
    if (Get-Command -Name corepack -Type Application -ErrorAction SilentlyContinue) {
        Write-Output 'Run ''corepack enable'' to install the Yarn shim for this Node.js installation.'
    } else {
        Write-Output 'Install a Node.js distribution that includes corepack, then run ''corepack enable''.'
    }
    $missing = $true
}

if ($missing) {
    exit 1
}

Write-Output "Node.js $(& node --version) and Yarn $(& yarn --version) are available."
