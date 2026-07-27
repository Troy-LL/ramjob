# Call from repo root before cargo:  . .\scripts\dev-env.ps1
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

$vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" `
  -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
  -property installationPath 2>$null
if ($vs) {
  $vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"
  if (Test-Path $vcvars) {
    cmd /c "`"$vcvars`" >nul && set" | ForEach-Object {
      if ($_ -match '^(.*?)=(.*)$') {
        [System.Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
      }
    }
  }
}

$sdkRoot = Join-Path $env:LOCALAPPDATA "Temp\winsdk-nupkg\extracted"
if (Test-Path (Join-Path $sdkRoot "c\um\x64\kernel32.Lib")) {
  $lib = @(
    (Join-Path $sdkRoot "c\um\x64"),
    (Join-Path $sdkRoot "c\ucrt\x64")
  ) -join ';'
  $inc = @(
    (Join-Path $sdkRoot "c\um"),
    (Join-Path $sdkRoot "c\ucrt"),
    (Join-Path $sdkRoot "c\shared")
  ) -join ';'
  if ($env:LIB) { $env:LIB = "$lib;$env:LIB" } else { $env:LIB = $lib }
  if ($env:INCLUDE) { $env:INCLUDE = "$inc;$env:INCLUDE" } else { $env:INCLUDE = $inc }
}

Write-Host "dev-env: cargo=$(Get-Command cargo -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source)"
Write-Host "dev-env: link=$(Get-Command link.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source)"
