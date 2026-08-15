<#
.SYNOPSIS
    Audits — and optionally restores — the files that the resource extractor (§4.6)
    pulled out of installed mods.

.DESCRIPTION
    The extractor routes "ancillary" files (docs, templates, root images) of an
    imported mod to <library>\resources\<category>\<id>\... instead of the game
    content. That classification was too aggressive for cars and tracks: real AC
    assets living at the root of the mod folder (body_shadow.png, tyre_N_shadow.png,
    logo.png, map.png, ...) were moved out of the mod folder, which the golden rule
    forbids — nothing inside the mod folder may ever be taken away.

    This script lists everything currently sitting in <library>\resources and tells,
    for each file, whether it came from INSIDE a mod folder (restorable) or from
    OUTSIDE any mod folder (legitimate ancillary file, left alone).

    Origin is derived from the category, not guessed:
      cars / tracks / skins / track_skins / sounds / apps
                -> extracted from inside that mod's own folder  => INSIDE
      others    -> leftover picked up next to the mods in the archive => OUTSIDE

    With -Restore, INSIDE files are COPIED back into the mod folder (originals are
    kept in resources; nothing is deleted). Existing files are never overwritten.

.PARAMETER Library
    One or more library roots. Defaults to library_path from
    %APPDATA%\com.pitbox.app\config.json.

.PARAMETER Restore
    Copy the INSIDE files back into their mod folder. Without it the script only
    reports (dry run).

.PARAMETER IncludeOthers
    Also list the resources\others\... files (reported as OUTSIDE, never restored).

.PARAMETER Csv
    Optional path to also write the full table as CSV.

.EXAMPLE
    pwsh -File scripts\audit-resources.ps1
    pwsh -File scripts\audit-resources.ps1 -Library D:\Games\AC-Lib -Csv report.csv
    pwsh -File scripts\audit-resources.ps1 -Library D:\Games\AC-Lib -Restore
#>
[CmdletBinding()]
param(
    [string[]]$Library,
    [switch]$Restore,
    [switch]$IncludeOthers,
    [string]$Csv
)

$ErrorActionPreference = 'Stop'

# How many path segments after resources\<category>\ identify the mod, and where
# the mod is stored in the library. Mirrors resources::resources_dir_for.
$Categories = @{
    'cars'        = @{ IdDepth = 1; Store = 'cars';        Origin = 'inside' }
    'tracks'      = @{ IdDepth = 1; Store = 'tracks';      Origin = 'inside' }
    'skins'       = @{ IdDepth = 2; Store = 'skins';       Origin = 'inside' }
    'track_skins' = @{ IdDepth = 2; Store = 'track_skins'; Origin = 'inside' }
    'sounds'      = @{ IdDepth = 2; Store = 'sounds';      Origin = 'inside' }
    'apps'        = @{ IdDepth = 1; Store = 'apps';        Origin = 'inside' }
    'others'      = @{ IdDepth = 1; Store = 'others';      Origin = 'outside' }
}

function Resolve-Libraries {
    param([string[]]$Given)
    if ($Given) { return $Given }
    $cfg = Join-Path $env:APPDATA 'com.pitbox.app\config.json'
    if (-not (Test-Path $cfg)) { throw "config.json not found at $cfg — pass -Library explicitly." }
    $path = (Get-Content $cfg -Raw | ConvertFrom-Json).library_path
    if (-not $path) { throw "library_path is empty in $cfg — pass -Library explicitly." }
    return , $path
}

# A mod folder that holds only sub-directories and no file of its own is a
# versioned mod (cars/tracks): the real content lives one level down, in every
# version folder. Anything else receives the file at its own root.
function Get-TargetRoots {
    param([string]$ModDir, [string]$Category)
    if (-not (Test-Path -LiteralPath $ModDir)) { return @() }
    if ($Category -eq 'others' -or $Category -eq 'apps') { return @($ModDir) }
    $children = Get-ChildItem -LiteralPath $ModDir -Force
    $hasFiles = @($children | Where-Object { -not $_.PSIsContainer }).Count -gt 0
    $dirs = @($children | Where-Object { $_.PSIsContainer })
    if (-not $hasFiles -and $dirs.Count -gt 0) { return @($dirs | ForEach-Object { $_.FullName }) }
    return @($ModDir)
}

$rows = New-Object System.Collections.Generic.List[object]

foreach ($libRaw in (Resolve-Libraries $Library)) {
    $lib = (Resolve-Path -LiteralPath $libRaw -ErrorAction SilentlyContinue)
    if (-not $lib) { Write-Warning "Library not found: $libRaw"; continue }
    $lib = $lib.Path
    $resRoot = Join-Path $lib 'resources'
    if (-not (Test-Path -LiteralPath $resRoot)) { Write-Warning "No resources folder in $lib"; continue }

    foreach ($file in Get-ChildItem -LiteralPath $resRoot -Recurse -File -Force) {
        $rel = $file.FullName.Substring($resRoot.Length + 1)
        $parts = $rel -split '\\'
        $category = $parts[0]
        $meta = $Categories[$category]
        if (-not $meta) {
            $rows.Add([pscustomobject]@{
                    Library = $lib; Category = $category; Mod = ''; File = $rel
                    SizeKB  = [math]::Round($file.Length / 1KB, 1); Origin = 'unknown'
                    Status  = 'unknown category'; Target = ''
                })
            continue
        }
        if (-not $IncludeOthers -and $meta.Origin -eq 'outside') { continue }

        $depth = [Math]::Min($meta.IdDepth, $parts.Count - 2)
        if ($depth -lt 1) { $depth = 1 }
        $modId = ($parts[1..$depth]) -join '\'
        $inner = ($parts[($depth + 1)..($parts.Count - 1)]) -join '\'
        $modDir = Join-Path $lib (Join-Path $meta.Store $modId)

        if ($meta.Origin -eq 'outside') {
            $rows.Add([pscustomobject]@{
                    Library = $lib; Category = $category; Mod = $modId; File = $inner
                    SizeKB  = [math]::Round($file.Length / 1KB, 1); Origin = 'outside'
                    Status  = 'ancillary (kept)'; Target = ''
                })
            continue
        }

        $roots = Get-TargetRoots -ModDir $modDir -Category $category
        if ($roots.Count -eq 0) {
            $rows.Add([pscustomobject]@{
                    Library = $lib; Category = $category; Mod = $modId; File = $inner
                    SizeKB  = [math]::Round($file.Length / 1KB, 1); Origin = 'inside'
                    Status  = 'mod folder not found'; Target = $modDir
                })
            continue
        }

        foreach ($root in $roots) {
            $target = Join-Path $root $inner
            $status = if (Test-Path -LiteralPath $target) { 'already present' } else { 'MISSING from mod' }
            if ($Restore -and $status -eq 'MISSING from mod') {
                $dir = Split-Path -Parent $target
                if (-not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
                Copy-Item -LiteralPath $file.FullName -Destination $target
                $status = 'RESTORED'
            }
            $rows.Add([pscustomobject]@{
                    Library = $lib; Category = $category; Mod = $modId; File = $inner
                    SizeKB  = [math]::Round($file.Length / 1KB, 1); Origin = 'inside'
                    Version = (Split-Path -Leaf $root); Status = $status; Target = $target
                })
        }
    }
}

$rows | Sort-Object Category, Mod, File | Format-Table Category, Mod, Version, File, SizeKB, Status -AutoSize

''
'Summary'
'-------'
$rows | Group-Object Status | Sort-Object Count -Descending | ForEach-Object { '  {0,-22} {1}' -f $_.Name, $_.Count }
'  {0,-22} {1}' -f 'files listed', $rows.Count
$affected = @($rows | Where-Object { $_.Status -in @('MISSING from mod', 'RESTORED') } | Select-Object -ExpandProperty Mod -Unique)
'  {0,-22} {1}' -f 'mods affected', $affected.Count

if ($Csv) {
    $rows | Sort-Object Category, Mod, File | Export-Csv -LiteralPath $Csv -NoTypeInformation -Encoding UTF8
    "`nCSV written to $Csv"
}

if (-not $Restore -and ($rows | Where-Object Status -eq 'MISSING from mod')) {
    "`nDry run. Re-run with -Restore to copy those files back into the mod folders."
    "Files are copied, never moved: the resources folder is left untouched."
    "Deployed mods must be re-deployed (deactivate/reactivate) for the game to see them."
}
