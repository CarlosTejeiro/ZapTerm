# MSIX package for the Microsoft Store

The Store rejects unsigned MSI/EXE installers (policy 10.2.9) but signs an
MSIX package itself. So the Store version of ZapTerm is an MSIX; the MSI,
the setup EXE and the portable EXE on GitHub are unchanged.

- `AppxManifest.xml` — the package manifest. ZapTerm runs as an ordinary
  desktop program (`runFullTrust`), not sandboxed.
- `identity.json` — the package identity Partner Center assigns.
- `make-msix.ps1` — builds the package from a built `zapterm.exe`.

## Set-up, once

1. In Partner Center create the product as an **MSIX or PWA app** and reserve
   the name. (If the name is taken by the earlier EXE/MSI product, delete it
   there first.)
2. Open **Product management → Product identity** and copy into
   `identity.json`:
   - `Package/Identity/Name` → `name`
   - `Package/Identity/Publisher` → `publisher` (the whole `CN=…`)
   - `Package/Properties/PublisherDisplayName` → `publisherDisplayName`
3. From then on every release tag attaches `ZapTerm_<version>_x64.msix` to the
   GitHub release. Upload that file in the submission's **Packages** page.
   Until the identity is filled in, the release workflow skips the package.

The Store asks for the `runFullTrust` restricted capability to be justified
in the submission: ZapTerm is a desktop terminal that opens network
connections, serial ports and local shells.

## What is different for a Store install

- **Data folder.** Windows redirects what a packaged app writes under
  `%APPDATA%`. The Store version keeps its data in
  `%LOCALAPPDATA%\Packages\<package family>\LocalCache\Roaming\app.zapterm.client`
  (seen in the CI run), not in `%APPDATA%\app.zapterm.client` where the
  MSI/EXE install keeps it. Moving from one to the other: Settings → Backup,
  then restore.
- **Uninstalling removes the data** with the package.
- **Updates** come from the Store.
- The version is `X.Y.Z.0`: the Store requires the fourth number to be 0.

## Checking a change

`.github/workflows/msix.yml` runs on pull requests that touch this folder: it
builds the app on Windows, packs the MSIX, signs it with a throwaway
certificate (a package only installs when signed by its Publisher), installs
it, starts ZapTerm and checks it is still running after 20 seconds.

Not covered by that check: opening sessions, the local shell, and the vault's
"remember on this computer" inside the package. Try those on a real Windows
machine with the `ZapTerm-msix-test` artifact of the run before submitting.
