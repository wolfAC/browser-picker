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

## Develop

```sh
pnpm install
pnpm tauri dev                     # settings window; the picker needs a URL
pnpm tauri build                   # installers for the current OS
cd src-tauri && cargo test -- --ignored --nocapture   # list this machine's browsers and profiles
```

To be set as the default browser, the app must be **installed** (the built
`.app`, `.msi`/`.exe` or `.deb`/`.rpm`/AppImage), not running from `tauri dev`.
It stays running in the tray/menu bar so the picker opens instantly.
Settings are saved in the OS app-config directory as `config.json`.
