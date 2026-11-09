# Builds an unsigned MSIX package (optionally signs it with a dev certificate).
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\scripts\build-msix.ps1 -Version 0.1.0
#   powershell ... -CertificatePath .\dev.pfx -CertificatePassword secret
param(
    [string]$Version = "0.1.0",
    [string]$CertificatePath = "",
    [string]$CertificatePassword = ""
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }

# makeappx ships with the Windows SDK; fall back to the newest installed version.
$makeappx = (Get-Command makeappx.exe -ErrorAction SilentlyContinue).Source
if (-not $makeappx) {
    $kits = 'C:\Program Files (x86)\Windows Kits\10\bin'
    $candidate = Get-ChildItem $kits -Recurse -Filter makeappx.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if ($candidate) { $makeappx = $candidate.FullName }
}
if (-not $makeappx) {
    throw "makeappx.exe not found. Install the Windows SDK (App Certification Kit)."
}

$layout = Join-Path $root 'dist\msix-layout'
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
New-Item -ItemType Directory -Force $layout | Out-Null

Copy-Item (Join-Path $root 'target\release\statline.exe') $layout
Copy-Item (Join-Path $root 'packaging\msix\AppxManifest.xml') $layout
Copy-Item (Join-Path $root 'packaging\msix\Assets') $layout -Recurse
foreach ($file in @('LICENSE', 'PRIVACY.md', 'THIRD_PARTY_NOTICES.md')) {
    Copy-Item (Join-Path $root $file) $layout
}
New-Item -ItemType Directory -Force (Join-Path $layout 'docs') | Out-Null
Copy-Item (Join-Path $root 'docs\ACCESS.md') (Join-Path $layout 'docs')
Copy-Item (Join-Path $root 'licenses') $layout -Recurse

# The manifest version must be four parts; keep it in sync with -Version.
$manifestPath = Join-Path $layout 'AppxManifest.xml'
[xml]$manifest = Get-Content $manifestPath
$manifest.Package.Identity.Version = "$Version.0"
$manifest.Save($manifestPath)

$msix = Join-Path $root "dist\StatLine-$Version-x64.msix"
if (Test-Path $msix) { Remove-Item $msix -Force }
& $makeappx pack /d $layout /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }

if ($CertificatePath) {
    $signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
    if (-not $signtool) {
        $kits = 'C:\Program Files (x86)\Windows Kits\10\bin'
        $candidate = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) { $signtool = $candidate.FullName }
    }
    if (-not $signtool) { throw "signtool.exe not found; cannot sign" }

    $signArgs = @('sign', '/fd', 'SHA256', '/f', $CertificatePath)
    if ($CertificatePassword) { $signArgs += @('/p', $CertificatePassword) }
    $signArgs += $msix
    & $signtool @signArgs
    if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
}

Write-Output "msix package: $msix"
if (-not $CertificatePath) {
    Write-Output "unsigned: sideload requires a trusted certificate (see docs/ops/packaging.md)"
}
