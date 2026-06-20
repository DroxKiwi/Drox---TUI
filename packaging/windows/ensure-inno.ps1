#Requires -Version 5.1
<#
.SYNOPSIS
  Garantit ISCC.exe (Inno Setup 6) pour le build installateur Windows.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$ToolsDir
)

$ErrorActionPreference = 'Stop'

function Find-Iscc([string]$Root) {
    $globalPaths = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    )
    foreach ($path in $globalPaths) {
        if (Test-Path -LiteralPath $path) {
            return (Resolve-Path -LiteralPath $path).Path
        }
    }
    if (Test-Path -LiteralPath $Root) {
        $found = Get-ChildItem -Path $Root -Filter ISCC.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) { return $found.FullName }
    }
    return $null
}

$Iscc = Find-Iscc $ToolsDir
if ($Iscc) { return $Iscc }

New-Item -ItemType Directory -Force -Path $ToolsDir | Out-Null
$Installer = Join-Path $ToolsDir 'innosetup-installer.exe'
$Url = 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe'
$InstallDir = Join-Path $ToolsDir 'InnoSetup6'

Write-Host "Telechargement Inno Setup 6.7.3 (portable, $InstallDir)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $Url -OutFile $Installer -UseBasicParsing

$proc = Start-Process -FilePath $Installer -ArgumentList @(
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=$InstallDir"
) -Wait -PassThru
if ($proc.ExitCode -ne 0) {
    throw "Installation Inno Setup a echoue (code $($proc.ExitCode))"
}

Remove-Item -Force $Installer -ErrorAction SilentlyContinue

$Iscc = Find-Iscc $ToolsDir
if (-not $Iscc) {
    throw "ISCC.exe introuvable apres installation dans $ToolsDir"
}

Write-Host "Inno Setup pret: $Iscc" -ForegroundColor DarkGray
return $Iscc
