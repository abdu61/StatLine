# Builds the portable release folder and zip.
# Usage: powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1 -Version 0.1.0
param(
    [string]$Version = "0.1.0",
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $SkipBuild) {
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }
}

$exe = Join-Path $root 'target\release\statline.exe'
if (-not (Test-Path $exe)) { throw "missing $exe" }

$stage = Join-Path $root "dist\Statline-$Version-x64"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe $stage
foreach ($file in @('LICENSE', 'README.md', 'PRIVACY.md', 'THIRD_PARTY_NOTICES.md', 'CHANGELOG.md')) {
    Copy-Item (Join-Path $root $file) $stage
}
New-Item -ItemType Directory -Force (Join-Path $stage 'docs') | Out-Null
Copy-Item (Join-Path $root 'docs\ACCESS.md') (Join-Path $stage 'docs')
Copy-Item (Join-Path $root 'licenses') $stage -Recurse

$zip = Join-Path $root "dist\Statline-$Version-x64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -Path "$zip.sha256" -Value "$hash  Statline-$Version-x64.zip" -Encoding ASCII

Write-Output "portable package: $zip"
Write-Output "sha256: $hash"
