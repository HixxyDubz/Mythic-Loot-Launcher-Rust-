# Mythic Loot Launcher

The Tauri 2 rewrite of Mythic Loot Launcher. The native application core is Rust; the interface is React and TypeScript. It installs, verifies, updates and launches modpacks, with a planned Developer workflow for publishing those modpacks through GitHub.

This repository is deliberately separate from **MLLP** (Mythic Loot Launcher Python). MLLP remains the behavioral reference until each feature is independently ported and verified here.

## Current status

The application currently has a working native shell, server-free modpack profile store, built-in game catalogue, local installation detection, trusted v1 manifest validation, streaming SHA-256 file verification, readiness assessment, profile editing, native game launch, transactional modpack update/repair and restore-point recovery, Smart Launch orchestration, persistent Safe Launch recovery, and a local-first GitHub publishing workflow. Smart Launch checks every tracked file, chooses version update or changed-file repair through the same native transaction engine, requires explicit approval before live mutation, rechecks the complete manifest, and opens the game or selected Minecraft launcher only after success. Updates and repairs download or accept a trusted package, validate and stage it away from the live installation, preview the exact mutation, create a pre-change backup, apply only after explicit confirmation, post-verify every required file, and roll back on failure. Recovery history retains the newest five metadata-backed points; restoring one is independently staged, previewed, confirmed, protected by a second backup, verified and rollback-capable. Safe Launch moves only trusted-manifest optional files, records their exact hashes outside the installation, waits for the launched game process, and restores them after exit or through a guarded restart recovery. The Publisher can privacy-scan a source folder, build a deterministic ZIP and manifest, preview the exact release, and only then expose an explicitly confirmed GitHub Release action. The complete parity position is tracked in [docs/PARITY_STATUS.md](docs/PARITY_STATUS.md).

Game servers are deliberately out of scope. The launcher does not query, configure, start, stop or directly join them; players use the game or another application for that.

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
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Windows test artifacts:

```powershell
npm run package:windows
```

This produces an installable EXE at `artifacts/windows/Mythic Loot Launcher Setup <version>.exe` and a portable quick-test executable at `artifacts/windows/win-unpacked/Mythic Loot Launcher.exe`. Exact sizes and SHA-256 hashes are written to `artifacts/windows/build-manifest.json`.

For 7 Days to Die, the game directory is the Steam `7 Days To Die` folder and the managed modpack base folder is its `Mods` child. Publishing scans that `Mods` folder, so package and manifest paths remain relative to the location players actually update. Packages at or above 2 GiB are emitted as ordered 1 GiB GitHub assets with individual SHA-256 values; the updater verifies every part and the reconstructed ZIP. A read-only acceptance build of the measured 2.727 GiB live source completed successfully without contacting GitHub.

For Minecraft, CurseForge and Modrinth instances are supported sync targets. Settings can generate a small CurseForge profile-import ZIP or Modrinth `.mrpack` bootstrap from the trusted manifest. The bootstrap contains only the Minecraft version, loader identity, modpack name and version; it contains no mods, saves or owner metadata. Import it in the chosen launcher, select the resulting profile through **Detect installs**, then use **Smart Launch** or **Sync, update & repair**. The existing staged transaction applies only trusted release inventory; it does not synchronize saves, logs, screenshots, options, caches or launcher/account metadata. Current and legacy Modrinth profile locations are detected. Real import acceptance in both launchers remains pending.

Writable launcher state is stored in Tauri's local application data directory. Set `MYTHIC_LOOT_DATA_DIR` to an explicit folder for portable development or isolated tests.

## Safety boundary

Filesystem mutation, game detection, process launch, updates, backups, restores, Safe Launch recovery, and publishing belong in Rust. React is an untrusted presentation layer and only receives narrow Tauri commands. Publishing scans relative paths and text content without treating an incidental source-root username as a leak, excludes known runtime/private state, rejects credential-shaped data, and re-hashes cached native assets immediately before upload. Update sources must remain dedicated HTTPS/package locations; Discord invites are never downloads. The updater retains MLLP's stage, verify, backup, apply, post-verify, and rollback contract, caches native previews instead of accepting frontend paths at apply time, and refuses to stage inside the live modpack. Restore commands accept launcher-owned identifiers only, reject legacy or unsafe archives, and never accept an arbitrary frontend filesystem path. Safe Launch journals are launcher-owned, bound to the configured installation, and leave conflicts untouched for explicit recovery.
