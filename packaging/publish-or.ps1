#Requires -Version 5.1
<#
.SYNOPSIS
  Build release Windows, package, et publie vers le depot Official Release (OR).

.PARAMETER OrRepo
  Chemin du depot OR (defaut: ../Drox---TUI---OR).

.PARAMETER SkipBuild
  Reutilise le binaire deja compile.

.EXAMPLE
  .\packaging\publish-or.ps1
  .\packaging\publish-or.ps1 -OrRepo "C:\Users\coren\Desktop\GitHub\Drox---TUI---OR"
#>
[CmdletBinding()]
param(
    [string]$OrRepo = "",
    [switch]$SkipBuild,
    [switch]$SkipGitCommit
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path $PSScriptRoot -Parent
if (-not (Test-Path (Join-Path $RepoRoot 'drox\Cargo.toml'))) {
    throw "Depot source invalide: $RepoRoot"
}

if (-not $OrRepo) {
    $OrRepo = Join-Path (Split-Path $RepoRoot -Parent) 'Drox---TUI---OR'
}
if (Test-Path -LiteralPath $OrRepo) {
    $OrRepo = (Resolve-Path -LiteralPath $OrRepo).Path
} else {
    throw "Depot OR introuvable: $OrRepo"
}

Write-Host "==> Publication vers OR: $OrRepo" -ForegroundColor Cyan

$BuildScript = Join-Path $PSScriptRoot 'build-and-pack.ps1'
$result = & $BuildScript -SkipBuild:$SkipBuild
$Version = $result.Version
$SetupPath = $result.SetupPath
$ArtifactName = $result.ArtifactName
$Sha256 = $result.Sha256

$ReleaseDir = Join-Path $OrRepo "releases\v$Version"
$InstallWin = Join-Path $OrRepo 'install\windows'
$InstallLinux = Join-Path $OrRepo 'install\linux'

New-Item -ItemType Directory -Force -Path $ReleaseDir, $InstallWin, $InstallLinux | Out-Null

Copy-Item $SetupPath (Join-Path $ReleaseDir $ArtifactName) -Force
Copy-Item (Join-Path $RepoRoot "dist\SHA256SUMS-$Version-windows.txt") (Join-Path $ReleaseDir 'SHA256SUMS-windows.txt') -Force

Copy-Item (Join-Path $RepoRoot 'packaging\windows\install.ps1') (Join-Path $InstallWin 'install.ps1') -Force
Copy-Item (Join-Path $RepoRoot 'packaging\linux\install.sh') (Join-Path $InstallLinux 'install.sh') -Force

$ReleaseNotesPath = Join-Path $RepoRoot "docs\$Version\RELEASE_NOTES.md"
if (-not (Test-Path -LiteralPath $ReleaseNotesPath)) {
    $ReleaseNotesPath = Join-Path $RepoRoot 'docs\2.0.2\RELEASE_NOTES.md'
}
if (Test-Path -LiteralPath $ReleaseNotesPath) {
    $ReleaseNotes = [System.IO.File]::ReadAllText($ReleaseNotesPath)
    $ReleaseNotes += "`n`n---`n`n## Empreinte Windows`n`nSHA256 ``$ArtifactName`` : ``$Sha256```n"
} else {
$ReleaseNotes = @"
# Drox TUI v$Version

**Produit** : Drox TUI `$Version`  
**Moteur** : dérivé du moteur agent Drox IDE `1.5.0`  
**Certification** : local-first — pas de télémétrie Drox, pas de cloud obligatoire.

## Artefacts Windows x64

| Fichier | Description |
|---|---|
| ``drox-tui-$Version-windows-x64-setup.exe`` | Installateur Windows (assistant graphique) |
| ``SHA256SUMS-windows.txt`` | Empreinte SHA256 |

## Installation Windows

1. Telechargez ``drox-tui-$Version-windows-x64-setup.exe``
2. Lancez l'installateur (double-clic)
3. Cochez « Ajouter au PATH » si propose
4. Ouvrez un **nouveau** terminal :

``````powershell
drox-tui --workspace C:\chemin\projet
``````

## Linux x64

| Fichier | Description |
|---|---|
| ``drox-tui-$Version-linux-x64.tar.gz`` | Binaire + ``install.sh`` |
| ``SHA256SUMS-linux.txt`` | Empreinte SHA256 (si publie) |

``````bash
tar xzf drox-tui-$Version-linux-x64.tar.gz
cd drox-tui-$Version-linux-x64
./install.sh
drox-tui --workspace ~/projets/mon-repo
``````

Date de publication : $(Get-Date -Format 'yyyy-MM-dd')
"@
}

[System.IO.File]::WriteAllText((Join-Path $ReleaseDir 'RELEASE_NOTES.md'), $ReleaseNotes, (New-Object System.Text.UTF8Encoding $false))

$PublishedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'")
$ReleaseTag = "v$Version"
$GhBase = 'https://github.com/DroxKiwi/Drox---TUI---OR'
$WindowsUrl = "$GhBase/releases/download/$ReleaseTag/$ArtifactName"
$ReleaseNotesUrl = "$GhBase/blob/main/releases/v$Version/RELEASE_NOTES.md"

$latest = [ordered]@{
    version          = $Version
    published_at     = $PublishedAt
    product          = 'drox-tui'
    engine_baseline  = '1.5.0'
    windows_x64      = [ordered]@{
        url    = $WindowsUrl
        sha256 = $Sha256
    }
    release_notes    = $ReleaseNotesUrl
}

$LinuxTar = Join-Path $RepoRoot "dist\drox-tui-$Version-linux-x64.tar.gz"
if (Test-Path -LiteralPath $LinuxTar) {
    $LinuxName = "drox-tui-$Version-linux-x64.tar.gz"
    $LinuxHash = (Get-FileHash -LiteralPath $LinuxTar -Algorithm SHA256).Hash.ToLower()
    $latest.linux_x64 = [ordered]@{
        url    = "$GhBase/releases/download/$ReleaseTag/$LinuxName"
        sha256 = $LinuxHash
    }
}

$LatestJsonPath = Join-Path $OrRepo 'releases\latest.json'
New-Item -ItemType Directory -Force -Path (Split-Path $LatestJsonPath -Parent) | Out-Null
$JsonBody = ($latest | ConvertTo-Json -Depth 4) + "`n"
[System.IO.File]::WriteAllText($LatestJsonPath, $JsonBody, (New-Object System.Text.UTF8Encoding $false))

$PublicReadme = Join-Path $RepoRoot 'README.md'
if (-not (Test-Path -LiteralPath $PublicReadme)) {
    throw "README public introuvable: $PublicReadme"
}
Copy-Item $PublicReadme (Join-Path $OrRepo 'README.md') -Force
Write-Host "README public copie vers OR." -ForegroundColor DarkGray

Write-Host ""
Write-Host "Publie dans: $ReleaseDir" -ForegroundColor Green
Write-Host "  $ArtifactName"
Write-Host "  RELEASE_NOTES.md"
Write-Host "  SHA256SUMS-windows.txt"
Write-Host "  releases/latest.json"
Write-Host ""

if (-not $SkipGitCommit -and (Test-Path (Join-Path $OrRepo '.git'))) {
    Push-Location $OrRepo
    try {
        git add README.md releases install
        $status = git status --porcelain
        if ($status) {
            git commit -m "release: Drox TUI v$Version (Windows x64)"
            Write-Host "Commit OR cree. Lancez: git push" -ForegroundColor Yellow
        } else {
            Write-Host "Aucun changement git dans OR." -ForegroundColor DarkGray
        }
    } finally {
        Pop-Location
    }
}

Write-Host "Termine." -ForegroundColor Green
