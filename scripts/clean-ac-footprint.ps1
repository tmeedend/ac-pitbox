<#
.SYNOPSIS
    Lists — and optionally removes — everything Pit Box deployed into an
    Assetto Corsa install, so the game folder can be returned to a clean state.

.DESCRIPTION
    Only touches what is identifiable as ours, by construction:

      * reparse points (junctions / symlinks) — the app is the only thing that
        creates them under an AC install;
      * directories holding the `.pitbox-deployed.json` marker — the per-file
        hardlink deployments (§2). Removing the library first turns these into
        ordinary folders full of mod content that Steam will never clean up and
        that block re-activation later, which is exactly what this avoids.

    A real Kunos folder has neither, and is never touched.

    NOT covered, and deliberately so: the individual files a mod adds outside
    `content/<type>/<id>` — fonts, drivers, CSP configs, shaders. Once the
    database is gone, nothing on disk distinguishes them from files installed
    by Content Manager or by hand. They are inert: unclaimed files are never
    modified nor removed by the app, so leaving them costs nothing.

    Dry run by default. Nothing is deleted without -Apply.

.PARAMETER AcPath
    Assetto Corsa install root. Defaults to ac_install_path from
    %APPDATA%\com.pitbox.app\config.json.

.PARAMETER Apply
    Actually remove. Without it the script only reports.

.EXAMPLE
    pwsh -File scripts\clean-ac-footprint.ps1
    pwsh -File scripts\clean-ac-footprint.ps1 -AcPath "D:\SteamLibrary\steamapps\common\assettocorsa"
    pwsh -File scripts\clean-ac-footprint.ps1 -Apply
#>
[CmdletBinding()]
param(
    [string]$AcPath,
    [switch]$Apply
)

$ErrorActionPreference = 'Stop'
$MarkerFile = '.pitbox-deployed.json'

function Resolve-Ac {
    param([string]$Given)
    if ($Given) { return $Given }
    $cfg = Join-Path $env:APPDATA 'com.pitbox.app\config.json'
    if (-not (Test-Path $cfg)) { throw "config.json not found — pass -AcPath explicitly." }
    $p = (Get-Content $cfg -Raw | ConvertFrom-Json).ac_install_path
    if (-not $p) { throw "ac_install_path is empty in $cfg — pass -AcPath explicitly." }
    return $p
}

$ac = (Resolve-Path -LiteralPath (Resolve-Ac $AcPath)).Path
if (-not (Test-Path -LiteralPath (Join-Path $ac 'AssettoCorsa.exe'))) {
    throw "$ac does not look like an Assetto Corsa install (no AssettoCorsa.exe)."
}

# A reparse point, NOT `$_.LinkType` being set: LinkType also reads "HardLink"
# on an ordinary hardlinked file, so filtering on it listed every single file
# of a hardlink deployment — and would have flagged any Kunos file that happens
# to share an inode. The ReparsePoint attribute is the precise test.
function Test-ReparsePoint {
    param($Item)
    return [bool]($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)
}

# Depth 3 covers content\cars\<id>, content\tracks\<id>, apps\<kind>\<name>,
# the skin projections under content\<type>\<id>\skins, and the links a mod
# places at the AC root. Deeper is pointless: a reparse point is never nested
# inside another one we created.
$found = Get-ChildItem -LiteralPath $ac -Recurse -Force -Depth 3 -ErrorAction SilentlyContinue |
    Where-Object {
        (Test-ReparsePoint $_) -or
        ($_.PSIsContainer -and (Test-Path -LiteralPath (Join-Path $_.FullName $MarkerFile)))
    } |
    ForEach-Object {
        [pscustomobject]@{
            Kind   = if (Test-ReparsePoint $_) { 'link' } else { 'hardlinks' }
            Path   = $_.FullName
            Target = $_.Target
        }
    } |
    Sort-Object Kind, Path

if (-not $found) {
    "Nothing deployed by Pit Box found in $ac."
    return
}

$found | Format-Table Kind, Path -AutoSize
''
'{0,-12} {1}' -f 'links', @($found | Where-Object Kind -eq 'link').Count
'{0,-12} {1}' -f 'hardlinks', @($found | Where-Object Kind -eq 'hardlinks').Count

if (-not $Apply) {
    "`nDry run. Re-run with -Apply to remove these."
    "Library files are never touched: a link removal frees the game folder only."
    return
}

$removed = 0
$failed = @()
foreach ($e in $found) {
    try {
        if ($e.Kind -eq 'link') {
            # A directory reparse point needs Remove-Item -Recurse to detach;
            # the -Force keeps it from following into the target.
            $item = Get-Item -LiteralPath $e.Path -Force
            if ($item.PSIsContainer) { [System.IO.Directory]::Delete($e.Path, $false) }
            else { [System.IO.File]::Delete($e.Path) }
        }
        else {
            # Hardlink deployment: the entries are extra directory names for
            # data that also lives in the library, so deleting them frees the
            # game folder without destroying anything.
            Remove-Item -LiteralPath $e.Path -Recurse -Force
        }
        $removed++
    }
    catch {
        $failed += [pscustomobject]@{ Path = $e.Path; Error = $_.Exception.Message }
    }
}

"`n{0,-12} {1}" -f 'removed', $removed
if ($failed) {
    "{0,-12} {1}" -f 'failed', $failed.Count
    $failed | Format-Table -AutoSize
}
