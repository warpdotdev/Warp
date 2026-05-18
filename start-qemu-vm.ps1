param(
    [string]$Name = "basic-vm",
    [string]$IsoPath,
    [int]$DiskSizeGB = 30,
    [int]$MemoryMB = 4096,
    [int]$CpuCount = 2,
    [string]$VmRoot = (Join-Path $PSScriptRoot "vms"),
    [switch]$Install,
    [switch]$NoAccel
)

$ErrorActionPreference = "Stop"

function Require-Command {
    param([string]$CommandName)

    $command = Get-Command $CommandName -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "$CommandName was not found on PATH. Restart your shell or add the QEMU install directory to PATH."
    }

    return $command.Source
}

$qemuImg = Require-Command "qemu-img"
$qemuSystem = Require-Command "qemu-system-x86_64"

$vmDir = Join-Path $VmRoot $Name
$diskPath = Join-Path $vmDir "$Name.qcow2"

New-Item -ItemType Directory -Force -Path $vmDir | Out-Null

if (-not (Test-Path $diskPath)) {
    Write-Host "Creating disk: $diskPath ($DiskSizeGB GB)"
    & $qemuImg create -f qcow2 $diskPath "$($DiskSizeGB)G"
    if ($LASTEXITCODE -ne 0) {
        throw "qemu-img failed with exit code $LASTEXITCODE."
    }
} else {
    Write-Host "Using existing disk: $diskPath"
}

$qemuArgs = @(
    "-m", $MemoryMB,
    "-smp", $CpuCount,
    "-drive", "file=$diskPath,format=qcow2"
)

if (-not $NoAccel) {
    $qemuArgs = @("-accel", "whpx") + $qemuArgs
}

if ($IsoPath) {
    $resolvedIso = (Resolve-Path $IsoPath).Path
    if ($Install) {
        Write-Host "Booting installer ISO: $resolvedIso"
        $qemuArgs += @("-cdrom", $resolvedIso, "-boot", "d")
    } else {
        Write-Host "ISO provided but -Install was not set; booting from disk only."
    }
} elseif ($Install) {
    throw "-Install requires -IsoPath."
}

Write-Host "Starting VM '$Name' with $MemoryMB MB RAM and $CpuCount CPU(s)."
Write-Host "Press Ctrl+Alt+G to release mouse/keyboard capture from the QEMU window."

& $qemuSystem @qemuArgs
exit $LASTEXITCODE
