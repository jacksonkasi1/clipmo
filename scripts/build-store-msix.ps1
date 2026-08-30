[CmdletBinding()]
param(
    [switch]$DevelopmentSigned
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$manifestPath = Join-Path $repoRoot 'store\Package.appxmanifest'
$artifactRoot = Join-Path $repoRoot 'artifacts\store'
$stageRoot = Join-Path $artifactRoot 'stage-x64'
$outputName = if ($DevelopmentSigned) { 'Clipmo_0.2.10.0_x64-dev.msix' } else { 'Clipmo_0.2.10.0_x64.msix' }
$outputPath = Join-Path $artifactRoot $outputName
$targetTriple = if ($env:STORE_WINDOWS_TARGET) { $env:STORE_WINDOWS_TARGET } else { 'x86_64-pc-windows-msvc' }
if ($targetTriple -notin @('x86_64-pc-windows-msvc', 'x86_64-pc-windows-gnu')) {
    throw "Unsupported Store build target: $targetTriple"
}

function Resolve-WinApp {
    $command = Get-Command winapp -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    $localTool = Join-Path $repoRoot 'artifacts\tools\winappcli\x64\winapp.exe'
    if (Test-Path -LiteralPath $localTool) { return $localTool }
    throw 'Microsoft winapp CLI is required. Install Microsoft.winappcli with winget or use microsoft/setup-WinAppCli in CI.'
}

$package = Get-Content (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json
$tauri = Get-Content (Join-Path $repoRoot 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
[xml]$manifest = Get-Content $manifestPath -Raw
$identity = $manifest.Package.Identity
$expectedMsixVersion = "$($package.version).0"
if ($package.version -ne $tauri.version) { throw 'package.json and Tauri versions differ.' }
if ($identity.Name -ne 'JacksonKasi.Clipmo') { throw 'Store package identity name changed.' }
if ($identity.Publisher -ne 'CN=B2D26A62-1D06-419E-B12A-D945916C46FD') { throw 'Store publisher identity changed.' }
if ($identity.Version -ne $expectedMsixVersion) { throw "MSIX version must be $expectedMsixVersion." }
if ($identity.ProcessorArchitecture -ne 'x64') { throw 'Store package must target x64.' }

Push-Location $repoRoot
try {
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $cargoBin = Join-Path $cargoHome 'bin'
    if (Test-Path -LiteralPath (Join-Path $cargoBin 'cargo.exe')) {
        $env:PATH = "$cargoBin;$env:PATH"
    } elseif ($env:RUSTUP_HOME) {
        $toolchainName = if ($targetTriple.EndsWith('-gnu')) { 'stable-x86_64-pc-windows-gnu' } else { 'stable-x86_64-pc-windows-msvc' }
        $directToolchain = Join-Path $env:RUSTUP_HOME "toolchains\$toolchainName\bin"
        if (Test-Path -LiteralPath (Join-Path $directToolchain 'cargo.exe')) {
            $cargoBin = $directToolchain
            $env:PATH = "$directToolchain;$env:PATH"
        }
    }
    & py -3 scripts/generate-store-assets.py
    if ($LASTEXITCODE -ne 0) { throw 'Store asset generation failed.' }

    & npx tauri build --ci --no-bundle --target $targetTriple
    if ($LASTEXITCODE -ne 0) { throw 'Tauri release build failed.' }

    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        $cargoExecutable = $cargo.Source
    } else {
        $cargoPath = Join-Path $cargoBin 'cargo.exe'
        if (-not (Test-Path -LiteralPath $cargoPath)) { throw 'Cargo executable was not found.' }
        $cargoExecutable = $cargoPath
    }
    $metadata = & $cargoExecutable metadata --format-version 1 --no-deps --manifest-path src-tauri/Cargo.toml | ConvertFrom-Json
    $releaseExe = Join-Path $metadata.target_directory "$targetTriple\release\clipmo.exe"
    if (-not (Test-Path -LiteralPath $releaseExe)) { throw "Missing Tauri executable: $releaseExe" }

    if (Test-Path -LiteralPath $stageRoot) { Remove-Item -LiteralPath $stageRoot -Recurse -Force }
    New-Item -ItemType Directory -Path $stageRoot -Force | Out-Null
    Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $stageRoot 'clipmo.exe')
    $webViewLoader = Join-Path (Split-Path $releaseExe -Parent) 'WebView2Loader.dll'
    if (Test-Path -LiteralPath $webViewLoader) {
        Copy-Item -LiteralPath $webViewLoader -Destination (Join-Path $stageRoot 'WebView2Loader.dll')
    }
    if (Test-Path -LiteralPath $outputPath) { Remove-Item -LiteralPath $outputPath -Force }

    $winapp = Resolve-WinApp
    $arguments = @('pack', $stageRoot, '--manifest', $manifestPath, '--executable', 'clipmo.exe', '--output', $outputPath)
    if ($DevelopmentSigned) {
        $certificateRoot = Join-Path $artifactRoot 'development-certificate'
        New-Item -ItemType Directory -Path $certificateRoot -Force | Out-Null
        $certificate = Join-Path $certificateRoot 'devcert.pfx'
        if (-not (Test-Path -LiteralPath $certificate)) {
            Push-Location $certificateRoot
            try {
                & $winapp cert generate --manifest $manifestPath --output $certificate --export-cer --if-exists skip
                if ($LASTEXITCODE -ne 0) { throw 'Development certificate generation failed.' }
            } finally { Pop-Location }
        }
        $arguments += @('--cert', $certificate)
    }
    & $winapp @arguments
    if ($LASTEXITCODE -ne 0) { throw 'winapp MSIX packaging failed.' }

    $validationArguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/validate-store-msix.ps1', '-PackagePath', $outputPath)
    if ($DevelopmentSigned) { $validationArguments += '-ExpectSigned' }
    & powershell @validationArguments
    if ($LASTEXITCODE -ne 0) { throw 'MSIX validation failed.' }
    Write-Host "Store MSIX: $outputPath"
} finally {
    Pop-Location
}
