# Sign a release ramjob-app.exe when a code-signing cert is available.
# Human-gated: purchase EV/OV cert first. Without cert, exits non-zero.
#
# Usage:
#   $env:RAMJOB_SIGN_CERT = "<thumbprint>"   # or path to .pfx
#   # optional: $env:RAMJOB_SIGN_PASSWORD for PFX
#   . .\scripts\dev-env.ps1
#   $env:CARGO_TARGET_DIR = "$env:USERPROFILE\ramjob-target"
#   .\scripts\sign-release.ps1

$ErrorActionPreference = "Stop"

$cert = $env:RAMJOB_SIGN_CERT
if ([string]::IsNullOrWhiteSpace($cert)) {
    Write-Error @"
Code signing cert required (SPEC OPEN: human-gated).
Set RAMJOB_SIGN_CERT to a certificate thumbprint or .pfx path, then re-run.
Example: `$env:RAMJOB_SIGN_CERT = 'ABC123...'
"@
    exit 1
}

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path (Get-Location) "target" }
$exe = Join-Path $targetDir "release\ramjob-app.exe"
if (-not (Test-Path $exe)) {
    Write-Error "Missing release binary: $exe`nBuild first: cargo build -p ramjob-app --release"
    exit 1
}

$signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signtool) {
    $kits = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
    if (Test-Path $kits) {
        $found = Get-ChildItem -Path $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\' } |
            Select-Object -First 1
        if ($found) { $signtool = $found }
    }
}
if (-not $signtool) {
    Write-Error "signtool.exe not found. Install Windows SDK Signing Tools."
    exit 1
}

$args = @(
    "sign", "/fd", "SHA256", "/tr", "http://timestamp.digicert.com", "/td", "SHA256"
)
if (Test-Path $cert) {
    $args += @("/f", $cert)
    if ($env:RAMJOB_SIGN_PASSWORD) {
        $args += @("/p", $env:RAMJOB_SIGN_PASSWORD)
    }
} else {
    $args += @("/sha1", $cert)
}
$args += $exe

Write-Host "Signing $exe ..."
& $signtool.Source @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Signed OK."
