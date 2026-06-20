#Requires -Version 5.1
<#
.SYNOPSIS
  Installe drox-tui.exe (release officielle Drox TUI).

.DESCRIPTION
  Copie le binaire dans %LOCALAPPDATA%\Programs\DroxTUI\bin
  et ajoute ce dossier au PATH utilisateur (optionnel).

.PARAMETER InstallDir
  Dossier racine d'installation (defaut: %LOCALAPPDATA%\Programs\DroxTUI).

.PARAMETER NoPath
  N'ajoute pas au PATH utilisateur.
#>
[CmdletBinding()]
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\DroxTUI",
    [switch]$NoPath
)

$ErrorActionPreference = 'Stop'

$SourceExe = Join-Path $PSScriptRoot 'drox-tui.exe'
if (-not (Test-Path $SourceExe)) {
    Write-Error "drox-tui.exe introuvable a cote de install.ps1 ($SourceExe)"
}

$BinDir = Join-Path $InstallDir 'bin'
$DestExe = Join-Path $BinDir 'drox-tui.exe'

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Copy-Item -Path $SourceExe -Destination $DestExe -Force

$VersionFile = Join-Path $PSScriptRoot 'VERSION'
if (Test-Path $VersionFile) {
    Copy-Item -Path $VersionFile -Destination (Join-Path $InstallDir 'VERSION') -Force
}

if (-not $NoPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($null -eq $userPath) { $userPath = '' }
    $parts = $userPath -split ';' | Where-Object { $_ -and ($_ -ne $BinDir) }
    if ($parts -notcontains $BinDir) {
        $newPath = ($parts + $BinDir) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        $env:Path = "$env:Path;$BinDir"
    }
}

Write-Host ""
Write-Host "Drox TUI installe dans: $DestExe" -ForegroundColor Green
if (-not $NoPath) {
    Write-Host "PATH utilisateur mis a jour. Ouvrez un nouveau terminal." -ForegroundColor Yellow
}
Write-Host ""
Write-Host "Lancement:" -ForegroundColor Cyan
Write-Host "  drox-tui --workspace C:\chemin\vers\projet"
Write-Host ""
