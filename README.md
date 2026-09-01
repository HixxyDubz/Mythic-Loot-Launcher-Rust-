# Mythic Loot Launcher

The Tauri 2 rewrite of Mythic Loot Launcher. The native application core is Rust; the interface is React and TypeScript. The Player edition installs, verifies, updates and launches modpacks. The separate Developer edition adds the reviewed GitHub publishing workflow.

This repository is deliberately separate from **MLLP** (Mythic Loot Launcher Python). MLLP remains the behavioral reference until each feature is independently ported and verified here.

## Current status

The application currently has a working native shell, server-free modpack profile store, Developer-only modpack creation, built-in game catalogue, local installation detection, trusted v1 manifest validation, streaming SHA-256 file verification, readiness assessment, profile editing, native game launch, transactional modpack update/repair and restore-point recovery, Smart Launch orchestration, persistent Safe Launch recovery, and a local-first GitHub publishing workflow. Smart Launch checks every tracked file, chooses version update or changed-file repair through the same native transaction engine, requires explicit approval before live mutation, rechecks the complete manifest, and opens the game or selected Minecraft launcher only after success. Updates and repairs download or accept a trusted package, validate and stage it away from the live installation, preview the exact mutation, create a pre-change backup, apply only after explicit confirmation, post-verify every required file, and roll back on failure. Recovery history retains the newest five metadata-backed points; restoring one is independently staged, previewed, confirmed, protected by a second backup, verified and rollback-capable. Safe Launch moves only trusted-manifest optional files, records their exact hashes outside the installation, waits for the launched game process, and restores them after exit or through a guarded restart recovery. The Developer Publisher can privacy-scan a source folder, build a deterministic ZIP and manifest, preview the exact release, and only then expose an explicitly confirmed GitHub Release action. A successful release updates that local profile to the new latest-manifest URL and makes it eligible for a separately previewed, privacy-bounded Player catalogue publication. Player is compiled without that UI, public-metadata editing, or those native commands. Production has no browser-preview profiles or fallback launcher state: every API path requires the native Rust application and fails closed otherwise. The complete parity position is tracked in [docs/PARITY_STATUS.md](docs/PARITY_STATUS.md).

Player startup renders from bundled and last-verified local data immediately, then refreshes the public [`launcher-catalog.json`](launcher-catalog.json) from the launcher's public `main` branch and each modpack's dedicated manifest asynchronously from GitHub. The catalogue never contains machine-local paths or launcher/account state. Remote metadata is size-bounded and schema/path/identity validated before atomic cache replacement; offline or invalid responses leave the previous verified data active. Developer treats its local profile metadata as authoritative: new packs begin as hidden drafts, a successful release enables catalogue visibility, and unchecking visibility archives a pack from the next reviewed catalogue publication.

Game servers are deliberately out of scope. The launcher does not query, configure, start, stop or directly join them; players use the game or another application for that.

The Player and Developer editions include the same native Support workspace. It discovers Minecraft and 7DTD logs only from known locations, reads a bounded tail, redacts personal paths, account identity, obvious secrets and network identifiers, and shows the exact summary and excerpt before any file is created. Export requires explicit confirmation and writes only the reviewed summary plus optional redacted excerpt under launcher-owned data. Server configuration, options and full game configuration files are never included.

## Development

Prerequisites on Windows: Node.js, Rust, Microsoft C++ Build Tools, and WebView2.

```powershell
npm install
npm run tauri dev
```

Verification:

```powershell
npm run test
npm run build
npm run verify:no-mock-data
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Windows test artifacts:

```powershell
npm run package:windows
```

This produces four artifacts:

- `artifacts/windows/player/Mythic Loot Launcher Player Setup <version>.exe`
- `artifacts/windows/player/win-unpacked/Mythic Loot Launcher Player.exe`
- `artifacts/windows/developer/Mythic Loot Launcher Developer Setup <version>.exe`
- `artifacts/windows/developer/win-unpacked/Mythic Loot Launcher Developer.exe`

Use `npm run package:windows:player` or `npm run package:windows:developer` to rebuild one edition. Exact sizes and SHA-256 hashes are written to `artifacts/windows/build-manifest.json`. The editions use distinct Windows application identifiers and data directories so they can be installed and tested side by side.

For 7 Days to Die, the game directory is the Steam `7 Days To Die` folder and the managed modpack base folder is its `Mods` child. Publishing scans that `Mods` folder, so package and manifest paths remain relative to the location players actually update. Packages at or above 2 GiB are emitted as ordered 1 GiB GitHub assets with individual SHA-256 values; the updater verifies every part and the reconstructed ZIP. A read-only acceptance build of the measured 2.727 GiB live source completed successfully without contacting GitHub.

For Minecraft, CurseForge and Modrinth instances are supported sync targets. Settings can generate a small CurseForge profile-import ZIP or Modrinth `.mrpack` bootstrap from the trusted manifest. The bootstrap contains only the Minecraft version, loader identity, modpack name and version; it contains no mods, saves or owner metadata. Import it in the chosen launcher, select the resulting profile through **Detect installs**, then use **Smart Launch** or **Sync, update & repair**. The existing staged transaction applies only trusted release inventory; it does not synchronize saves, logs, screenshots, options, caches or launcher/account metadata. Current and legacy Modrinth profile locations are detected. Real import acceptance in both launchers remains pending.

Writable launcher state is stored in Tauri's local application data directory. Set `MYTHIC_LOOT_DATA_DIR` to an explicit folder for portable development or isolated tests.

## Safety boundary

Filesystem mutation, game detection, process launch, updates, backups, restores, Safe Launch recovery, support-log discovery/redaction/export, and Developer publishing belong in Rust. React is an untrusted presentation layer and only receives narrow Tauri commands. There is no production mock-data module or non-native success fallback; browser execution reports that the native desktop application is required. Test inputs live only behind test-file imports and the Windows packaging script rejects former preview symbols or strings in production source and each built frontend. Player is compiled without the Publisher route and without the Rust publisher/packager modules or command registrations; this is not a runtime toggle. Developer publishing scans relative paths and text content without treating an incidental source-root username as a leak, excludes known runtime/private state, rejects credential-shaped data, and re-hashes cached native assets immediately before upload. Update sources must remain dedicated HTTPS/package locations; Discord invites are never downloads. The updater retains MLLP's stage, verify, backup, apply, post-verify, and rollback contract, caches native transaction previews instead of accepting frontend paths at apply time, and refuses to stage inside the live modpack. Restore commands accept launcher-owned identifiers only, reject legacy or unsafe archives, and never accept an arbitrary frontend filesystem path. Safe Launch journals are launcher-owned, bound to the configured installation, and leave conflicts untouched for explicit recovery.
