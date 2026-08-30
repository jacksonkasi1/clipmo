[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackagePath,
    [switch]$ExpectSigned
)

$ErrorActionPreference = 'Stop'
$resolvedPackage = (Resolve-Path -LiteralPath $PackagePath).Path
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead($resolvedPackage)
$hasPackageSignature = $false
try {
    $manifestEntry = $archive.GetEntry('AppxManifest.xml')
    if (-not $manifestEntry) { $manifestEntry = $archive.GetEntry('Package.appxmanifest') }
    if (-not $manifestEntry) { throw 'MSIX does not contain an Appx manifest.' }
    $reader = [IO.StreamReader]::new($manifestEntry.Open())
    try { [xml]$manifest = $reader.ReadToEnd() } finally { $reader.Dispose() }

    $identity = $manifest.Package.Identity
    if ($identity.Name -ne 'JacksonKasi.Clipmo') { throw "Unexpected identity name: $($identity.Name)" }
    if ($identity.Publisher -ne 'CN=B2D26A62-1D06-419E-B12A-D945916C46FD') { throw "Unexpected publisher: $($identity.Publisher)" }
    if ($identity.Version -ne '0.2.10.0') { throw "Unexpected version: $($identity.Version)" }
    if ($identity.ProcessorArchitecture -ne 'x64') { throw "Unexpected architecture: $($identity.ProcessorArchitecture)" }
    if (-not $archive.GetEntry('clipmo.exe')) { throw 'MSIX is missing clipmo.exe.' }
    $hasPackageSignature = $null -ne $archive.GetEntry('AppxSignature.p7x')

    $payload = @($archive.Entries | Where-Object { -not $_.FullName.EndsWith('/') } | ForEach-Object FullName)
    $allowed = @('clipmo.exe', 'WebView2Loader.dll', 'AppxManifest.xml', 'Package.appxmanifest', 'AppxBlockMap.xml', 'AppxSignature.p7x', '[Content_Types].xml', 'resources.pri', 'pri.resfiles', 'priconfig.xml')
    $unexpected = @($payload | Where-Object { $_ -notlike 'Assets/*' -and $_ -notlike 'AppxMetadata/*' -and $_ -notin $allowed })
    if ($unexpected) { throw "Unexpected staged payload: $($unexpected -join ', ')" }
} finally {
    $archive.Dispose()
}

if ($ExpectSigned) {
    if (-not $hasPackageSignature) { throw 'Expected a development signature.' }
    $signature = Get-AuthenticodeSignature -LiteralPath $resolvedPackage
    if ($signature.Status -eq 'NotSigned' -or -not $signature.SignerCertificate) { throw 'Expected a development signature.' }
    if ($signature.SignerCertificate.Subject -ne 'CN=B2D26A62-1D06-419E-B12A-D945916C46FD') { throw 'Development signer does not match the manifest publisher.' }
    Write-Host "Validated $resolvedPackage (development signed)"
} else {
    if ($hasPackageSignature) { throw 'Store-upload MSIX must be unsigned.' }
    Write-Host "Validated $resolvedPackage (unsigned)"
}
