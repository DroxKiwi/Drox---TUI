#Requires -Version 5.1
<#
.SYNOPSIS
  Localise les fichiers splash / startup dans un fork VS Code Drox.

.PARAMETER IdeRepo
  Chemin racine du dépôt IDE (fork VS Code).

.EXAMPLE
  .\docs\animation-start\extract-splash-from-ide.ps1 -IdeRepo "C:\dev\Drox-IDE"
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$IdeRepo
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $IdeRepo)) {
    throw "Depot IDE introuvable: $IdeRepo"
}

$ReportDir = Join-Path $PSScriptRoot 'reports'
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null
$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$ReportPath = Join-Path $ReportDir "ide-splash-scan-$Stamp.txt"

$Patterns = @(
    'splash',
    'gettingStarted',
    'GettingStarted',
    'startup',
    'welcomePage',
    'windowTitle',
    'workbench.startup'
)

$Lines = [System.Collections.Generic.List[string]]::new()
$Lines.Add("Drox IDE splash scan")
$Lines.Add("Repo: $IdeRepo")
$Lines.Add("Date: $(Get-Date -Format o)")
$Lines.Add('')

foreach ($pat in $Patterns) {
    $Lines.Add("=== pattern: $pat ===")
    $hits = Get-ChildItem -LiteralPath $IdeRepo -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object {
            $_.FullName -notmatch '\\(node_modules|\.git|out|dist|target)\\' -and
            ($_.Name -match $pat -or (Select-String -LiteralPath $_.FullName -Pattern $pat -Quiet -ErrorAction SilentlyContinue))
        } |
        Select-Object -First 40
    if ($hits) {
        foreach ($h in $hits) {
            $rel = $h.FullName.Substring($IdeRepo.Length).TrimStart('\')
            $Lines.Add("  $rel")
        }
    } else {
        $Lines.Add('  (aucun hit)')
    }
    $Lines.Add('')
}

$ProductJson = Join-Path $IdeRepo 'product.json'
if (Test-Path -LiteralPath $ProductJson) {
    $Lines.Add('=== product.json (extrait branding) ===')
    $json = Get-Content -LiteralPath $ProductJson -Raw | ConvertFrom-Json
    foreach ($key in @('nameShort', 'nameLong', 'applicationName', 'windowTitle', 'welcomePage')) {
        if ($json.PSObject.Properties.Name -contains $key) {
            $Lines.Add("  $key = $($json.$key)")
        }
    }
    $Lines.Add('')
}

[System.IO.File]::WriteAllLines($ReportPath, $Lines)
Write-Host "Rapport: $ReportPath" -ForegroundColor Green

# Copie assets TUI de reference a cote du rapport
$RefDir = Join-Path $ReportDir "drox-tui-reference-$Stamp"
New-Item -ItemType Directory -Force -Path $RefDir | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'drox-logo.ascii.txt') $RefDir -Force
Copy-Item (Join-Path $PSScriptRoot 'drox-phosphor-theme.json') $RefDir -Force
Copy-Item (Join-Path $PSScriptRoot 'drox-splash-spec.ts') $RefDir -Force
Write-Host "Reference TUI copiee: $RefDir" -ForegroundColor DarkGray
