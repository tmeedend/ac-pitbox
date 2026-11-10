<#
.SYNOPSIS
  Proof of concept: renders protected cars inside Assetto Corsa with the
  pitbox_shot CSP app (docs/csp-shot-research.md).

.DESCRIPTION
  Developer tool, never shipped. Links the app into apps\lua with a junction
  (no game file is written or replaced), writes the job file in
  Documents\Assetto Corsa\pitbox_shot, and starts a Quick Drive practice
  through Content Manager, which stays the owner of race.ini (golden rule 4).
  The app renders, writes done.json and closes the game.

  -Remove takes the junction out again. Nothing else is left in the game folder.

.EXAMPLE
  ./scripts/poc-csp-shot.ps1 -Car ddm_honda_s2000_ap1
  ./scripts/poc-csp-shot.ps1 -Remove
#>
param(
  [string]$Car,
  [string]$Track = 'magione',
  [string]$Skin,
  [switch]$Remove
)
$ErrorActionPreference = 'Stop'

$config = Get-Content "$env:APPDATA\com.pitbox.app\config.json" -Raw | ConvertFrom-Json
$ac = $config.ac_install_path
$cm = $config.content_manager_exe
$link = Join-Path $ac 'apps\lua\pitbox_shot'
$source = Join-Path $PSScriptRoot '..\src-tauri\resources\csp\pitbox_shot' | Resolve-Path

function Test-Junction($path) {
  $item = Get-Item $path -Force -ErrorAction SilentlyContinue
  return $item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
}

if ($Remove) {
  if (Test-Junction $link) {
    # A junction is removed as a directory entry: its target is untouched.
    [IO.Directory]::Delete($link)
    "junction removed: $link"
  } elseif (Test-Path $link) {
    throw "$link is a real folder, not ours: left alone"
  }
  return
}
if (-not $Car) { throw '-Car is required' }

if (-not (Test-Path $link)) {
  New-Item -ItemType Junction -Path $link -Target $source | Out-Null
  "junction created: $link -> $source"
} elseif (-not (Test-Junction $link)) {
  throw "$link exists and is a real folder: refusing to touch it"
}

$docs = [Environment]::GetFolderPath('MyDocuments')
$jobDir = Join-Path $docs 'Assetto Corsa\pitbox_shot'
New-Item -ItemType Directory -Force -Path (Join-Path $jobDir 'out') | Out-Null
Remove-Item (Join-Path $jobDir 'done.json') -ErrorAction SilentlyContinue

# The Catalogue preset (src/lib/gridthumbs/gridThumbPrefs.svelte.ts).
$job = [ordered]@{
  cars        = @(@{ id = $Car })
  template    = @{ azimuth = 320; elevation = 8; fov = 26; margin = 6; height = -7 }
  width       = 1024
  height      = 576
  supersample = 2
  warmup      = 8
  quit        = $true
  probes      = @{
    localAppData = Join-Path $env:LOCALAPPDATA 'com.pitbox.app\pitbox_shot_probe.txt'
    temp         = Join-Path $env:TEMP 'pitbox_shot_probe.txt'
  }
}
$job | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM (Join-Path $jobDir 'job.json')

# Quick Drive practice, pit start: the car stands still in its pit box.
$preset = [ordered]@{
  Mode               = '/Pages/Drive/QuickDrive_Practice.xaml'
  ModeData           = '{"StartType":"PIT","Penalties":false,"PlayerBallast":0,"PlayerRestrictor":0}'
  CarId              = $Car
  TrackId            = $Track
  WeatherId          = '3_clear'
  RealConditions     = $false
  Temperature        = 22.0
  Time               = 46800
  TimeMultipler      = 1
  tpc                = $true
  TrackPropertiesData = '{"s":1.0,"t":1.0,"r":0.0,"g":1,"w":false}'
  asc                = $false
}
$presetFile = Join-Path $env:TEMP "pitbox-shot-$([guid]::NewGuid()).json"
$preset | ConvertTo-Json | Set-Content -Encoding utf8NoBOM $presetFile
$raceIni = Join-Path $docs 'Assetto Corsa\cfg\race.ini'
$baseline = (Get-Item $raceIni).LastWriteTimeUtc
Start-Process -FilePath $cm -ArgumentList "acmanager://race/quick?presetFile=$presetFile"

# The Quick Drive preset carries no skin, so CM picks one. Same remedy as Pit
# Box's own launch (raceini::spawn_player_skin_patcher): rewrite SKIN in
# [CAR_0] right after CM wrote race.ini, before the game reads it.
if ($Skin) {
  $deadline = (Get-Date).AddSeconds(60)
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 50
    if ((Get-Item $raceIni).LastWriteTimeUtc -eq $baseline) { continue }
    $ini = [IO.File]::ReadAllText($raceIni)
    # The player's car is MODEL= of [RACE]; [CAR_0] holds "MODEL=-".
    if ($ini -notmatch "(?ms)^\[RACE\][^\[]*^MODEL=$([regex]::Escape($Car))\s*$") { continue }
    $patched = [regex]::Replace($ini, '(?ms)(^\[CAR_0\][^\[]*?^SKIN=)[^\r\n]*', "`${1}$Skin")
    [IO.File]::WriteAllText($raceIni, $patched)
    "race.ini: skin set to $Skin"
    break
  }
}
"session started for $Car on $Track; results in $jobDir"
