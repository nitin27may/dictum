# Windows Code Signing & Installer Guide

This document covers how to sign and distribute the Dictum Windows installer once you have a code signing certificate.

---

## Prerequisites

- A **Windows EV or OV Code Signing Certificate** (see options below)
- Node.js 18+, Rust toolchain, Visual Studio Build Tools 2022 (C++ workload)
- GitHub Actions (for CI builds) or a local Windows machine

---

## Option A: Azure Trusted Signing (Recommended for CI)

Azure Trusted Signing is the cheapest and most CI-friendly option (~$10/month).

### 1. Setup in Azure Portal

1. Create an **Azure Trusted Signing** account in the Azure Portal
2. Create a **Certificate Profile** (choose "Public Trust" for public distribution)
3. Create an **Identity Validation** request — Azure verifies your organization
4. Note down:
   - `Endpoint` (e.g., `https://eus.codesigning.azure.net`)
   - `Account Name`
   - `Certificate Profile Name`

### 2. Create an Azure Service Principal

```bash
az ad sp create-for-rbac --name "dictum-signing" --role contributor \
  --scopes /subscriptions/<SUBSCRIPTION_ID>/resourceGroups/<RESOURCE_GROUP>
```

Save the output — you'll need `clientId`, `clientSecret`, and `tenantId`.

### 3. Add GitHub Secrets

Go to your repo → Settings → Secrets and variables → Actions. Add:

| Secret Name | Value |
|---|---|
| `AZURE_TENANT_ID` | From service principal output |
| `AZURE_CLIENT_ID` | From service principal output |
| `AZURE_CLIENT_SECRET` | From service principal output |
| `AZURE_ENDPOINT` | e.g., `https://eus.codesigning.azure.net` |
| `AZURE_CODE_SIGNING_ACCOUNT` | Your Trusted Signing account name |
| `AZURE_CERT_PROFILE` | Your certificate profile name |

### 4. Tauri Configuration

No changes needed in `tauri.conf.json` — signing happens as a post-build step (see GitHub Actions workflow below).

---

## Option B: DigiCert / Sectigo EV Certificate with KeyLocker

If using a traditional EV certificate with cloud HSM (e.g., DigiCert KeyLocker):

### 1. Obtain the Certificate

Your IT team provides:
- **Certificate file** (`.pfx` or `.p12`)
- **Certificate password**
- Or cloud HSM API credentials (DigiCert KeyLocker, SSL.com eSigner)

### 2. Add GitHub Secrets

| Secret Name | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` file (see encoding step below) |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for the `.pfx` file |

#### Encode the certificate to Base64:

```bash
# On macOS/Linux
base64 -i certificate.pfx -o certificate-base64.txt

# On Windows (PowerShell)
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certificate.pfx")) | Out-File certificate-base64.txt
```

Copy the contents of `certificate-base64.txt` into the `WINDOWS_CERTIFICATE` secret.

### 3. Tauri Configuration

Add to `src-tauri/tauri.conf.json` inside the `bundle` key:

```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "<CERT_THUMBPRINT>",
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.digicert.com"
    }
  }
}
```

> Get the thumbprint from: `certutil -dump certificate.pfx` or your CA's dashboard.

---

## Option C: Local USB Token (Not CI-friendly)

If your company provides a **physical USB hardware token** (e.g., SafeNet eToken):

1. Install the token drivers on a Windows machine
2. Install the certificate from the token into the Windows Certificate Store
3. Build locally:

```bash
npm run tauri build
```

4. Sign manually using `signtool`:

```powershell
signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 ^
  /a "src-tauri\target\release\bundle\nsis\Dictum_0.1.0_x64-setup.exe"
```

> This approach requires a human with the USB token plugged in — not suitable for CI/CD.

---

## Tauri Bundle Configuration

Update `src-tauri/tauri.conf.json` to configure Windows installer options:

```json
{
  "bundle": {
    "active": true,
    "targets": ["nsis", "msi"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.ico"
    ],
    "windows": {
      "nsis": {
        "installMode": "both",
        "displayLanguageSelector": false,
        "installerIcon": "icons/icon.ico",
        "headerImage": "icons/nsis-header.bmp",
        "sidebarImage": "icons/nsis-sidebar.bmp"
      },
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      }
    }
  }
}
```

### Key settings explained:
- **`targets`**: `"nsis"` = .exe installer (recommended), `"msi"` = .msi package
- **`installMode`**: `"both"` lets user choose per-user or per-machine install
- **`webviewInstallMode`**: `"embedBootstrapper"` bundles WebView2 installer for offline installs
- **`icon.ico`**: Windows requires `.ico` format — generate from PNG using an online converter or `icotool`

---

## GitHub Actions Workflow

Create `.github/workflows/build-windows.yml`:

### For Azure Trusted Signing:

```yaml
name: Build Windows Installer

on:
  push:
    tags: ['v*']
  workflow_dispatch:

jobs:
  build:
    runs-on: windows-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install frontend dependencies
        run: npm ci

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'Dictum v__VERSION__'
          releaseBody: 'See release notes for details.'
          releaseDraft: true

      - name: Install Azure SignTool
        run: dotnet tool install --global AzureSignTool

      - name: Sign NSIS installer
        run: |
          AzureSignTool sign -kvu "${{ secrets.AZURE_ENDPOINT }}" `
            -kva "${{ secrets.AZURE_CLIENT_ID }}" `
            -kvs "${{ secrets.AZURE_CLIENT_SECRET }}" `
            -kvt "${{ secrets.AZURE_TENANT_ID }}" `
            -kvc "${{ secrets.AZURE_CERT_PROFILE }}" `
            -tr http://timestamp.digicert.com -td sha256 `
            "src-tauri/target/release/bundle/nsis/Dictum_*_x64-setup.exe"

      - name: Upload signed installer
        uses: actions/upload-artifact@v4
        with:
          name: dictum-windows-installer
          path: src-tauri/target/release/bundle/nsis/*.exe
```

### For PFX Certificate (DigiCert/Sectigo):

```yaml
name: Build Windows Installer

on:
  push:
    tags: ['v*']
  workflow_dispatch:

jobs:
  build:
    runs-on: windows-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Import certificate
        env:
          WINDOWS_CERTIFICATE: ${{ secrets.WINDOWS_CERTIFICATE }}
          WINDOWS_CERTIFICATE_PASSWORD: ${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}
        run: |
          $pfxBytes = [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE)
          [IO.File]::WriteAllBytes("certificate.pfx", $pfxBytes)
          Import-PfxCertificate -FilePath certificate.pfx `
            -CertStoreLocation Cert:\CurrentUser\My `
            -Password (ConvertTo-SecureString -String $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force)
          Remove-Item certificate.pfx

      - name: Install frontend dependencies
        run: npm ci

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'Dictum v__VERSION__'
          releaseBody: 'See release notes for details.'
          releaseDraft: true

      - name: Upload installer
        uses: actions/upload-artifact@v4
        with:
          name: dictum-windows-installer
          path: src-tauri/target/release/bundle/nsis/*.exe
```

---

## Windows Icon Generation

Tauri needs an `.ico` file for Windows. Generate one from your existing PNG icons:

```bash
# Using ImageMagick
convert icons/1024x1024.png -define icon:auto-resize=256,128,64,48,32,16 icons/icon.ico

# Or use https://convertio.co/png-ico/ for a quick online conversion
```

Add `"icons/icon.ico"` to the `bundle.icon` array in `tauri.conf.json`.

---

## Testing Without a Certificate

During development, you can build unsigned installers:

```bash
# On a Windows machine or VM
npm run tauri build
```

The unsigned `.exe` will be at:
```
src-tauri/target/release/bundle/nsis/Dictum_0.1.0_x64-setup.exe
```

> Windows SmartScreen will warn users about unsigned apps. They can click **"More info" → "Run anyway"** to proceed.

---

## Checklist

- [ ] Obtain code signing certificate (Azure Trusted Signing, EV, or OV)
- [ ] Generate `icon.ico` and add to `tauri.conf.json`
- [ ] Add NSIS/MSI targets to `tauri.conf.json` bundle config
- [ ] Configure GitHub Secrets with certificate credentials
- [ ] Create GitHub Actions workflow (`.github/workflows/build-windows.yml`)
- [ ] Test build by pushing a tag: `git tag v0.1.0 && git push origin v0.1.0`
- [ ] Verify installer works on a clean Windows machine
- [ ] Verify SmartScreen shows your company name (not "Unknown publisher")

---

## Troubleshooting

| Issue | Solution |
|---|---|
| SmartScreen still warns after signing | OV certs need reputation — submit to Microsoft SmartScreen portal or use EV cert |
| WebView2 missing on target machine | Set `webviewInstallMode` to `embedBootstrapper` in tauri.conf.json |
| Build fails with "linker not found" | Install Visual Studio Build Tools 2022 with "Desktop development with C++" workload |
| Signing fails in CI | Check that all secrets are set correctly; verify cert is not expired |
| NSIS error "Output folder does not exist" | Ensure `npm run build` (frontend) completes before Tauri build |
