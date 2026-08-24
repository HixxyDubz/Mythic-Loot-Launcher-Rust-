# Mythic Loot Launcher

The Tauri 2 rewrite of Mythic Loot Launcher. The native application core is Rust; the interface is React and TypeScript. It installs, verifies, updates and launches modpacks, with a planned Developer workflow for publishing those modpacks through GitHub.

This repository is deliberately separate from **MLLP** (Mythic Loot Launcher Python). MLLP remains the behavioral reference until each feature is independently ported and verified here.

## Current status

The application currently has a working native shell, server-free modpack profile store, built-in game catalogue, local installation detection, trusted v1 manifest validation, streaming SHA-256 file verification, readiness assessment, profile editing, native game launch, and a local-first GitHub publishing workflow. The Publisher can privacy-scan a source folder, build a deterministic ZIP and manifest, preview the exact release, and only then expose an explicitly confirmed GitHub Release action. The complete parity position is tracked in [docs/PARITY_STATUS.md](docs/PARITY_STATUS.md).

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

Writable launcher state is stored in Tauri's local application data directory. Set `MYTHIC_LOOT_DATA_DIR` to an explicit folder for portable development or isolated tests.

## Safety boundary

Filesystem mutation, game detection, process launch, updates, backups, and publishing belong in Rust. React is an untrusted presentation layer and only receives narrow Tauri commands. Publishing scans relative paths and text content without treating an incidental source-root username as a leak, excludes known runtime/private state, rejects credential-shaped data, and re-hashes cached native assets immediately before upload. Update sources must remain dedicated HTTPS/package locations; Discord invites are never downloads. Any future updater must retain MLLP's stage, verify, backup, apply, post-verify, and rollback contract.
