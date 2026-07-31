#Requires -Version 5.1
<#
.SYNOPSIS
  verify-ramjob helper — doctor / list / gate / run-once / cleanup
.NOTES
  Invoke from repo root. Sources scripts\dev-env.ps1. Sets CARGO_TARGET_DIR when unset
  and E: (or non-profile) trees hit Smart App Control blocks.
#>
param(
  [Parameter(Position = 0, Mandatory = $true)]
  [ValidateSet("doctor", "list", "gate", "run-once", "cleanup")]
  [string]$Command,

  [string]$EvidenceDir = "",

  [int]$Mb = 64,

  [int]$HoldSecs = 8
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")
Set-Location $RepoRoot

function Ensure-DevEnv {
  . (Join-Path $RepoRoot "scripts\dev-env.ps1")
  if (-not $env:CARGO_TARGET_DIR) {
    $env:CARGO_TARGET_DIR = Join-Path $env:USERPROFILE "ramjob-target"
  }
}

function Get-RamjobExe {
  $candidates = @(
    (Join-Path $env:CARGO_TARGET_DIR "debug\ramjob.exe"),
    (Join-Path $RepoRoot "target\debug\ramjob.exe")
  )
  foreach ($c in $candidates) {
    if (Test-Path $c) { return (Resolve-Path $c).Path }
  }
  return $null
}

function Get-HogExe {
  $candidates = @(
    (Join-Path $env:CARGO_TARGET_DIR "debug\ramjob-hog.exe"),
    (Join-Path $RepoRoot "target\debug\ramjob-hog.exe")
  )
  foreach ($c in $candidates) {
    if (Test-Path $c) { return (Resolve-Path $c).Path }
  }
  return $null
}

function Ensure-EvidenceDir {
  if (-not $EvidenceDir) {
    throw "EvidenceDir is required for this command"
  }
  New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
  $script:Scratch = Join-Path $EvidenceDir "scratch"
  New-Item -ItemType Directory -Force -Path $script:Scratch | Out-Null
}

function Append-Pid([int]$ProcessId) {
  $pidsFile = Join-Path $EvidenceDir "pids.txt"
  Add-Content -Path $pidsFile -Value $ProcessId
}

function Write-Meta([string]$FeatureId, [string]$Extra) {
  $head = (git -C $RepoRoot rev-parse --short HEAD 2>$null)
  if (-not $head) { $head = "unknown" }
  @"
run_dir=$EvidenceDir
feature=$FeatureId
git_head=$head
cargo_target_dir=$env:CARGO_TARGET_DIR
$Extra
"@ | Set-Content -Path (Join-Path $EvidenceDir "meta.txt") -Encoding utf8
}

switch ($Command) {
  "doctor" {
    Ensure-DevEnv
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    $link = Get-Command link.exe -ErrorAction SilentlyContinue
    $ramjob = Get-RamjobExe
    if (-not $ramjob) {
      Write-Host "building ramjob-cli…"
      cargo build -p ramjob-cli | Out-Host
      $ramjob = Get-RamjobExe
    }
    $ok = $true
    if ($cargo) { Write-Host "cargo=ok $($cargo.Source)" } else { Write-Host "cargo=missing"; $ok = $false }
    if ($link) { Write-Host "msvc_link=ok $($link.Source)" } else { Write-Host "msvc_link=missing"; $ok = $false }
    if ($ramjob) {
      Write-Host "ramjob=ok $ramjob"
      & $ramjob list | Out-Null
      if ($LASTEXITCODE -ne 0) { Write-Host "ramjob_list=fail exit=$LASTEXITCODE"; $ok = $false }
      else { Write-Host "ramjob_list=ok" }
    } else {
      Write-Host "ramjob=missing"
      $ok = $false
    }
    if (-not $ok) { exit 1 }
    exit 0
  }

  "list" {
    Ensure-DevEnv
    Ensure-EvidenceDir
    cargo build -p ramjob-cli | Out-Host
    $ramjob = Get-RamjobExe
    if (-not $ramjob) { throw "ramjob.exe not found after build" }
    $outFile = Join-Path $EvidenceDir "list.stdout.txt"
    $stderrFile = Join-Path $EvidenceDir "list.stderr.txt"
    $psi = @"
& '$ramjob' list 1> '$outFile' 2> '$stderrFile'
exit `$LASTEXITCODE
"@
    $proc = Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoProfile", "-Command", $psi) -Wait -PassThru
    $exit = $proc.ExitCode
    Write-Meta -FeatureId "list-gf" -Extra "command=$ramjob list`nexit=$exit"
    if ($exit -ne 0) {
      Write-Host "list failed exit=$exit"
      Get-Content $stderrFile -ErrorAction SilentlyContinue | Write-Host
      exit $exit
    }
    $rows = Get-Content $outFile | Where-Object { $_.Trim() -ne "" }
    if ($rows.Count -lt 1) {
      Write-Host "list validation failed: no rows"
      exit 1
    }
    foreach ($line in $rows) {
      $parts = $line -split "`t"
      if ($parts.Count -ne 3) {
        Write-Host "list validation failed: bad row: $line"
        exit 1
      }
    }
    Write-Host "list=ok rows=$($rows.Count)"
    Write-Host "evidence=$outFile"
    exit 0
  }

  "gate" {
    Ensure-DevEnv
    Ensure-EvidenceDir
    cargo build -p ramjob-cli -p ramjob-hog | Out-Host
    $ramjob = Get-RamjobExe
    $hog = Get-HogExe
    if (-not $ramjob -or -not $hog) { throw "ramjob or ramjob-hog missing after build" }

    $hogLog = Join-Path $script:Scratch "hog.log"
    $hogProc = Start-Process -FilePath $hog -ArgumentList @("--mode", "forget", "--mb", "$Mb", "--hold-secs", "$HoldSecs") `
      -RedirectStandardOutput $hogLog -RedirectStandardError $hogLog -PassThru -WindowStyle Hidden
    Append-Pid $hogProc.Id
    Start-Sleep -Seconds 1

    $gateOut = Join-Path $EvidenceDir "gate-out.md"
    $stdout = Join-Path $EvidenceDir "gate.stdout.txt"
    $stderr = Join-Path $EvidenceDir "gate.stderr.txt"
    $args = @("gate", "--image", "ramjob-hog", "--out", $gateOut, "--wait-secs", "15")
    $p = Start-Process -FilePath $ramjob -ArgumentList $args -Wait -PassThru `
      -RedirectStandardOutput $stdout -RedirectStandardError $stderr -NoNewWindow
    Write-Meta -FeatureId "gate-ry" -Extra "command=$ramjob $($args -join ' ')`nexit=$($p.ExitCode)`nhog_pid=$($hogProc.Id)"

    if (-not $hogProc.HasExited) {
      Stop-Process -Id $hogProc.Id -Force -ErrorAction SilentlyContinue
    }

    if ($p.ExitCode -ne 0) {
      Write-Host "gate failed exit=$($p.ExitCode)"
      Get-Content $stderr -ErrorAction SilentlyContinue | Write-Host
      exit $p.ExitCode
    }
    $text = Get-Content $stdout -Raw
    if ($text -notmatch "Classification:") {
      Write-Host "gate validation failed: missing Classification"
      exit 1
    }
    if (-not (Test-Path $gateOut)) {
      Write-Host "gate validation failed: missing --out file"
      exit 1
    }
    Write-Host "gate=ok"
    Write-Host "evidence=$stdout"
    exit 0
  }

  "run-once" {
    Ensure-DevEnv
    Ensure-EvidenceDir
    cargo build -p ramjob-cli | Out-Host
    $ramjob = Get-RamjobExe
    if (-not $ramjob) { throw "ramjob.exe not found" }
    $cfg = Join-Path $EvidenceDir "config.verify.toml"
    @"
version = 2
runaway_multiplier = 3.0
"@ | Set-Content -Path $cfg -Encoding utf8
    $stdout = Join-Path $EvidenceDir "run.stdout.txt"
    $stderr = Join-Path $EvidenceDir "run.stderr.txt"
    $args = @("run", "--once", "--simulate-armed", "--config", $cfg)
    $p = Start-Process -FilePath $ramjob -ArgumentList $args -Wait -PassThru `
      -RedirectStandardOutput $stdout -RedirectStandardError $stderr -NoNewWindow
    Write-Meta -FeatureId "run-once" -Extra "command=$ramjob $($args -join ' ')`nexit=$($p.ExitCode)"
    if ($p.ExitCode -ne 0) {
      Write-Host "run-once failed exit=$($p.ExitCode)"
      Get-Content $stderr -ErrorAction SilentlyContinue | Write-Host
      exit $p.ExitCode
    }
    $text = Get-Content $stdout -Raw
    if ($text -notmatch "tick system=") {
      Write-Host "run-once validation failed: missing tick line"
      exit 1
    }
    Write-Host "run-once=ok"
    Write-Host "evidence=$stdout"
    exit 0
  }

  "cleanup" {
    if (-not $EvidenceDir) { throw "EvidenceDir is required" }
    $pidsFile = Join-Path $EvidenceDir "pids.txt"
    if (Test-Path $pidsFile) {
      Get-Content $pidsFile | ForEach-Object {
        $id = 0
        if ([int]::TryParse($_.Trim(), [ref]$id) -and $id -gt 0) {
          Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
          Write-Host "stopped pid=$id"
        }
      }
    }
    $scratch = Join-Path $EvidenceDir "scratch"
    if (Test-Path $scratch) {
      Remove-Item -Recurse -Force $scratch
      Write-Host "removed scratch"
    }
    Write-Host "cleanup=ok (proof files retained under $EvidenceDir)"
    exit 0
  }
}
