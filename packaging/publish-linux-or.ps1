#Requires -Version 5.1
<#
.SYNOPSIS
  Publie l'archive Linux x64 vers le depot OR (seul ou en complement Windows).

.PARAMETER OrRepo
  Chemin du depot OR (defaut: ../Drox---TUI---OR).

.PARAMETER SkipBuild
  Reutilise dist/drox-tui-<version>-linux-x64.tar.gz deja produit.

.PARAMETER WslDistro
  Distribution WSL (defaut: Ubuntu).

.EXAMPLE
  .\packaging\publish-linux-or.ps1
  .\packaging\publish-linux-or.ps1 -SkipBuild
#>
[CmdletBinding()]
param(
    [string]$OrRepo = "",
    [switch]$SkipBuild,
    [string]$WslDistro = "Ubuntu",
    [switch]$SkipGitCommit
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path $PSScriptRoot -Parent

if (-not $OrRepo) {
    $OrRepo = Join-Path (Split-Path $RepoRoot -Parent) 'Drox---TUI---OR'
}
$OrRepo = (Resolve-Path -LiteralPath $OrRepo).Path

$toml = Get-Content (Join-Path $RepoRoot 'drox\Cargo.toml') -Raw
if ($toml -notmatch '\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"') {
    throw 'Version workspace introuvable'
}
$Version = $Matches[1]
$LinuxTar = Join-Path $RepoRoot "dist\drox-tui-$Version-linux-x64.tar.gz"
$LinuxName = "drox-tui-$Version-linux-x64.tar.gz"

function ConvertTo-WslPath {
    param([string]$WindowsPath)
    $normalized = (Resolve-Path -LiteralPath $WindowsPath).Path
    $drive = $normalized.Substring(0, 1).ToLower()
    $rest = $normalized.Substring(2).Replace('\', '/')
    return "/mnt/$drive$rest"
}

if (-not $SkipBuild) {
    if (-not (Get-Command wsl -ErrorAction SilentlyContinue)) {
        throw "WSL requis pour le build Linux depuis Windows. Sinon: ./packaging/build-and-pack-linux.sh sur une machine Linux."
    }
    Write-Host "==> Build Linux x64 (WSL $WslDistro)" -ForegroundColor Cyan
    $WslRepo = ConvertTo-WslPath $RepoRoot
    $SkipFlag = if ($SkipBuild) { '--skip-build' } else { '' }
    wsl -d $WslDistro bash -lc @"
set -euo pipefail
cd '$WslRepo'
sed -i 's/\r$//' packaging/build-and-pack-linux.sh packaging/linux/install.sh
source ~/.cargo/env 2>/dev/null || true
bash packaging/build-and-pack-linux.sh $SkipFlag
"@
}

if (-not (Test-Path -LiteralPath $LinuxTar)) {
    throw "Archive Linux absente: $LinuxTar"
}

$LinuxSha256 = (Get-FileHash -LiteralPath $LinuxTar -Algorithm SHA256).Hash.ToLower()
Write-Host "Archive Linux: $LinuxTar" -ForegroundColor Green
Write-Host "SHA256:        $LinuxSha256" -ForegroundColor DarkGray

$ReleaseDir = Join-Path $OrRepo "releases\v$Version"
$InstallLinux = Join-Path $OrRepo 'install\linux'
New-Item -ItemType Directory -Force -Path $ReleaseDir, $InstallLinux | Out-Null

Copy-Item $LinuxTar (Join-Path $ReleaseDir $LinuxName) -Force
$LinuxSums = Join-Path $RepoRoot "dist\SHA256SUMS-$Version-linux.txt"
if (Test-Path -LiteralPath $LinuxSums) {
    Copy-Item $LinuxSums (Join-Path $ReleaseDir 'SHA256SUMS-linux.txt') -Force
} else {
    [System.IO.File]::WriteAllText(
        (Join-Path $ReleaseDir 'SHA256SUMS-linux.txt'),
        "$LinuxSha256  $LinuxName`n",
        (New-Object System.Text.UTF8Encoding $false)
    )
}

Copy-Item (Join-Path $RepoRoot 'packaging\linux\install.sh') (Join-Path $InstallLinux 'install.sh') -Force

$ReleaseNotesPath = Join-Path $ReleaseDir 'RELEASE_NOTES.md'
if (-not (Test-Path -LiteralPath $ReleaseNotesPath)) {
    $SrcNotes = Join-Path $RepoRoot "docs\$Version\RELEASE_NOTES.md"
    if (Test-Path -LiteralPath $SrcNotes) {
        Copy-Item $SrcNotes $ReleaseNotesPath -Force
    }
}
if (Test-Path -LiteralPath $ReleaseNotesPath) {
    $notes = [System.IO.File]::ReadAllText($ReleaseNotesPath)
    if ($notes -notmatch [regex]::Escape($LinuxSha256)) {
        $notes += "`n`n---`n`n## Empreinte Linux`n`nSHA256 ``$LinuxName`` : ``$LinuxSha256```n"
        [System.IO.File]::WriteAllText($ReleaseNotesPath, $notes, (New-Object System.Text.UTF8Encoding $false))
    }
}

$LatestJsonPath = Join-Path $OrRepo 'releases\latest.json'
$GhBase = 'https://github.com/DroxKiwi/Drox---TUI---OR'
$ReleaseTag = "v$Version"

if (Test-Path -LiteralPath $LatestJsonPath) {
    $latest = Get-Content $LatestJsonPath -Raw | ConvertFrom-Json
} else {
    $latest = [ordered]@{
        version         = $Version
        published_at    = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'")
        product         = 'drox-tui'
        engine_baseline = '1.5.0'
        release_notes   = "$GhBase/blob/main/releases/v$Version/RELEASE_NOTES.md"
    }
}

$latest | Add-Member -NotePropertyName 'linux_x64' -NotePropertyValue ([ordered]@{
    url    = "$GhBase/releases/download/$ReleaseTag/$LinuxName"
    sha256 = $LinuxSha256
}) -Force

$latest.published_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'")
$JsonBody = ($latest | ConvertTo-Json -Depth 4) + "`n"
[System.IO.File]::WriteAllText($LatestJsonPath, $JsonBody, (New-Object System.Text.UTF8Encoding $false))

Write-Host ""
Write-Host "Publie dans: $ReleaseDir" -ForegroundColor Green
Write-Host "  $LinuxName"
Write-Host "  SHA256SUMS-linux.txt"
Write-Host "  releases/latest.json (linux_x64 ajoute)"
Write-Host ""

if (-not $SkipGitCommit -and (Test-Path (Join-Path $OrRepo '.git'))) {
    Push-Location $OrRepo
    try {
        git add releases install
        $status = git status --porcelain
        if ($status) {
            git commit -m "release: Drox TUI v$Version Linux x64"
            Write-Host "Commit OR cree. Puis: git push" -ForegroundColor Yellow
            Write-Host "Asset GitHub: gh release upload $ReleaseTag `"$ReleaseDir\$LinuxName`"" -ForegroundColor Yellow
        }
    } finally {
        Pop-Location
    }
}

Write-Host "Termine." -ForegroundColor Green
