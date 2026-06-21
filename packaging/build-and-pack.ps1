#Requires -Version 5.1
<#
.SYNOPSIS
  Compile drox-tui en release et produit l'installateur Windows x64 (.exe).

.PARAMETER SkipBuild
  Reutilise le binaire deja compile.

.EXAMPLE
  .\packaging\build-and-pack.ps1
  .\packaging\build-and-pack.ps1 -SkipBuild
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [string]$OutDir = ""
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path $PSScriptRoot -Parent
$DroxDir = Join-Path $RepoRoot 'drox'
$PackagingDir = Join-Path $RepoRoot 'packaging'

if (-not (Test-Path (Join-Path $DroxDir 'Cargo.toml'))) {
    throw "Depot invalide (drox/Cargo.toml absent): $RepoRoot"
}

$toml = Get-Content (Join-Path $DroxDir 'Cargo.toml') -Raw
if ($toml -notmatch '\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"') {
    throw 'Version workspace introuvable'
}
$Version = $Matches[1]

Write-Host "==> Drox TUI release $Version (Windows x64 - installateur)" -ForegroundColor Cyan

if (-not $SkipBuild) {
    Push-Location $DroxDir
    try {
        cargo build --release -p drox-tui
        if ($LASTEXITCODE -ne 0) { throw "cargo build a echoue ($LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
}

$Binary = Join-Path $DroxDir 'target\release\drox-tui.exe'
if (-not (Test-Path $Binary)) {
    throw "Binaire absent: $Binary - lancez sans -SkipBuild"
}

$DistRoot = if ($OutDir) { $OutDir } else { Join-Path $RepoRoot 'dist' }
$StageDir = Join-Path $DistRoot 'stage-windows'
$BinStage = Join-Path $StageDir 'bin'
$SetupBase = "drox-tui-$Version-windows-x64-setup"
$SetupPath = Join-Path $DistRoot "$SetupBase.exe"

if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path $BinStage | Out-Null

Copy-Item $Binary (Join-Path $BinStage 'drox-tui.exe')
Copy-Item (Join-Path $PackagingDir 'assets\drox.ico') (Join-Path $StageDir 'drox.ico')
Copy-Item (Join-Path $PackagingDir 'LICENSE-MIT.txt') (Join-Path $StageDir 'LICENSE')
Set-Content -Path (Join-Path $StageDir 'VERSION') -Value $Version -Encoding ASCII

$ToolsDir = Join-Path $PackagingDir 'tools'
$EnsureInno = Join-Path $PackagingDir 'windows\ensure-inno.ps1'
$Iscc = & $EnsureInno -ToolsDir $ToolsDir

$IssFile = Join-Path $PackagingDir 'windows\drox-tui-setup.iss'
Push-Location $RepoRoot
try {
    & $Iscc "/DMyAppVersion=$Version" $IssFile
    if ($LASTEXITCODE -ne 0) { throw "ISCC a echoue ($LASTEXITCODE)" }
} finally {
    Pop-Location
}

if (-not (Test-Path $SetupPath)) {
    throw "Installateur absent apres compilation: $SetupPath"
}

$Hash = (Get-FileHash -Path $SetupPath -Algorithm SHA256).Hash.ToLower()
$ChecksumFile = Join-Path $DistRoot "SHA256SUMS-$Version-windows.txt"
[System.IO.File]::WriteAllText($ChecksumFile, "$Hash  $SetupBase.exe`n", (New-Object System.Text.UTF8Encoding $false))

Write-Host ""
Write-Host "Installateur: $SetupPath" -ForegroundColor Green
Write-Host "SHA256:       $Hash"
Write-Host ""

return @{
    Version     = $Version
    SetupPath   = $SetupPath
    ArtifactName = "$SetupBase.exe"
    Sha256      = $Hash
}
