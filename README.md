# Browser Picker

Set it as your default browser. Every link you click in another app opens a small
picker at the cursor, listing each installed browser and each of its profiles.

Built with Tauri 2 (Rust) and React. Runs on macOS, Windows and Linux.

## Picker

- `1`–`9` open that row. `↑`/`↓` or `Tab` move, and `Enter` opens.
- Tick **Always open <site> with this choice**, or hold `Alt` while choosing, to
  skip the picker for that site and its subdomains next time.
- `⌘/Ctrl+C` copies the link. `Esc` or clicking elsewhere closes the picker.

## Supported browsers

Every browser the OS has registered for `https` links is listed. Profiles are
read for:

- **Chromium family** (`Local State`, opened with `--profile-directory`): Chrome
  (stable/beta/canary), Chromium, Brave, Edge, Vivaldi, Helium, Thorium
- **Firefox family** (`profiles.ini`, opened with `-P`): Firefox, Firefox
  Developer Edition, Zen, LibreWolf, Floorp, Waterfox

Other browsers (Safari, Arc, Opera and the rest) are listed without profiles.

| OS      | Browser discovery                               | Default-browser setup                                    |
| ------- | ----------------------------------------------- | -------------------------------------------------------- |
| macOS   | Scans app bundles that declare `https` in `Info.plist` | `LSSetDefaultHandlerForURLScheme`, which shows a system prompt |
| Windows | Reads `Clients\StartMenuInternet` (HKCU and HKLM) | Registers under HKCU, then opens Settings › Default apps |
| Linux   | Finds `.desktop` entries for `x-scheme-handler/https` (including Flatpak and Snap) | Writes a user `.desktop` file, then runs `xdg-settings` and `xdg-mime` |

## Updates

Installed copies check `https://github.com/wolfAC/browser-picker/releases/latest/download/latest.json`
30 seconds after launch and then every 6 hours. A new version downloads and installs in the
background, and the app restarts into it once no picker or settings window is open. You can also
use **Check for Updates…** in the tray menu or the button in Settings. Development builds never update.

Updates are signed. The private key is `~/.tauri/browser-picker.key` (keep it backed up, because
without it installed copies can't be updated). The public key is in `tauri.conf.json`.

### Release

```sh
scripts/release.sh 0.2.0
```

This bumps the version, tags it, builds and signs the universal macOS app, and publishes a GitHub
release with the DMG, the update archive and `latest.json`. `.github/workflows/release.yml` then
adds the Windows and Linux installers and their `latest.json` entries. The workflow needs the repo
secret `TAURI_SIGNING_PRIVATE_KEY` (`gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/browser-picker.key`).

## Develop

```sh
pnpm install
pnpm tauri dev                     # settings window; the picker needs a URL
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/browser-picker.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD= \
  pnpm tauri build                 # installers for the current OS (update archives need the key)
cd src-tauri && cargo test -- --ignored --nocapture   # list this machine's browsers and profiles
```

To be set as the default browser, the app must be **installed** (the built
`.app`, `.msi`/`.exe` or `.deb`/`.rpm`/AppImage), not running from `tauri dev`.
It stays running in the tray/menu bar so the picker opens instantly.
Settings are saved in the OS app-config directory as `config.json`.
