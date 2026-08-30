# Microsoft Store MSIX

Clipmo's Store package is an additional distribution path. The existing Tauri
NSIS configuration and Windows release workflow remain authoritative for the
standalone installer.

## Store identity

- Package name: `JacksonKasi.Clipmo`
- Publisher: `CN=B2D26A62-1D06-419E-B12A-D945916C46FD`
- Publisher display name: `Jackson Kasi`
- Store ID: `9P4FBGKV4GWC`
- Package family name: `JacksonKasi.Clipmo_f5z2yr12kw8xg`
- Version: `0.2.10.0`
- Architecture: `x64`

## Build

Install Microsoft's official winapp CLI, then run:

```powershell
npm run store:msix
```

The command builds the production frontend and x64 MSVC Tauri executable,
stages only the runtime payload, and creates the unsigned Store upload at:

```text
artifacts/store/Clipmo_0.2.10.0_x64.msix
```

The Microsoft Store signs the uploaded package. Do not sign this artifact.

The MSVC package contains only `clipmo.exe`; Windows supplies system DLLs and
the WebView2 Runtime. A local GNU fallback additionally stages
`WebView2Loader.dll`, which that target dynamically imports.

## Local installation test

From an elevated PowerShell session, run:

```powershell
npm run store:msix:test
```

This creates a self-signed development package under `artifacts/store`, trusts
its public certificate, installs and launches Clipmo, verifies the Partner
Center PFN, and uninstalls the package. The private key and certificate stay in
the ignored `artifacts/store/development-certificate` directory.

For identity testing without installing a certificate, stage a build and use
Microsoft's loose-layout command:

```powershell
winapp run artifacts/store/stage-x64 --manifest store/Package.appxmanifest --executable clipmo.exe --detach
winapp unregister --manifest store/Package.appxmanifest --force
```

## Data and capabilities

The package is a medium-integrity packaged classic app and requests:

- `runFullTrust`, required for Win32 clipboard monitoring, paste injection,
  tray integration, and global shortcuts;
- `privateNetworkClientServer`, required for Clipmo's LAN UDP discovery and
  TCP device synchronization.

No internet client capability is declared. Default MSIX AppData virtualization
is retained. This avoids the restricted `unvirtualizedResources` capability,
keeps new Store installations inside the MSIX lifecycle, and allows Windows to
fall back to Clipmo's existing real `%APPDATA%\app.clipdeck.desktop` data when
upgrading an existing standalone installation.

The manual GitHub Actions workflow `.github/workflows/store-msix.yml` builds and
uploads the unsigned MSIX. It does not publish to Partner Center.
