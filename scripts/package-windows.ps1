param([string]$Version = '0.1.0')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repoRoot
try {
    $env:CARGO_BUILD_JOBS = '1'
    cargo build --locked --release -p litecad-viewer --config profile.release.package.litecad-viewer.opt-level=1 --config profile.release.package.litecad-viewer.codegen-units=32
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repoRoot 'target' }
    $packageName = "LiteCAD-Viewer-$Version-windows-x64"
    $distRoot = Join-Path $repoRoot 'dist'
    $packageRoot = Join-Path $distRoot $packageName
    New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $targetRoot 'release\litecad-viewer.exe') -Destination (Join-Path $packageRoot 'LiteCADViewer.exe') -Force
    foreach ($name in @('LICENSE-MIT','LICENSE-APACHE','NOTICE','ATTRIBUTION.md','THIRD-PARTY-LICENSES.txt','README.md')) {
        Copy-Item -LiteralPath (Join-Path $repoRoot $name) -Destination $packageRoot -Force
    }
    Copy-Item -LiteralPath (Join-Path $repoRoot 'docs') -Destination $packageRoot -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot 'assets') -Destination $packageRoot -Recurse -Force
    $archive = Join-Path $distRoot "$packageName.zip"
    Compress-Archive -LiteralPath $packageRoot -DestinationPath $archive -Force
    $checksum = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$checksum  $packageName.zip" | Set-Content -LiteralPath (Join-Path $distRoot 'SHA256SUMS.txt') -Encoding ascii
    Write-Output $archive
} finally { Pop-Location }
