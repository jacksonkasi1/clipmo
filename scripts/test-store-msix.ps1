[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$artifactRoot = Join-Path $repoRoot 'artifacts\store'
$packagePath = Join-Path $artifactRoot 'Clipmo_0.2.10.0_x64-dev.msix'
$certificatePath = Join-Path $artifactRoot 'development-certificate\devcert.pfx'

Push-Location $repoRoot
try {
    & powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-store-msix.ps1 -DevelopmentSigned
    if ($LASTEXITCODE -ne 0) { throw 'Development MSIX build failed.' }

    $publicCertificate = Join-Path $artifactRoot 'development-certificate\devcert.cer'
    if (-not (Test-Path -LiteralPath $publicCertificate)) { throw 'Development public certificate was not generated.' }
    Write-Host 'Trusting the development certificate requires an elevated shell.'
    & certutil.exe -addstore -f TrustedPeople $publicCertificate
    if ($LASTEXITCODE -ne 0) { throw 'Could not trust the development certificate. Re-run this command from an elevated PowerShell session.' }

    $existing = Get-AppxPackage -Name 'JacksonKasi.Clipmo'
    if ($existing) { $existing | Remove-AppxPackage }
    Add-AppxPackage -Path $packagePath
    $installed = Get-AppxPackage -Name 'JacksonKasi.Clipmo'
    if (-not $installed) { throw 'Clipmo MSIX was not installed.' }
    if ($installed.PackageFamilyName -ne 'JacksonKasi.Clipmo_f5z2yr12kw8xg') { throw "Unexpected PFN: $($installed.PackageFamilyName)" }

    Start-Process explorer.exe 'shell:AppsFolder\JacksonKasi.Clipmo_f5z2yr12kw8xg!Clipmo'
    Start-Sleep -Seconds 5
    $running = Get-Process clipmo -ErrorAction SilentlyContinue
    if (-not $running) { throw 'Packaged Clipmo did not launch.' }
    $running | Stop-Process -Force
    $installed | Remove-AppxPackage
    if (Get-AppxPackage -Name 'JacksonKasi.Clipmo') { throw 'Clipmo MSIX did not uninstall cleanly.' }
    Write-Host 'Development MSIX install, launch, identity, and uninstall checks passed.'
} finally {
    Pop-Location
}
