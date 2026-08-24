# Mythic Loot Launcher

The Tauri 2 rewrite of Mythic Loot Launcher. The native application core is Rust; the interface is React and TypeScript.

This repository is deliberately separate from **MLLP** (Mythic Loot Launcher Python). MLLP remains the behavioral reference until each feature is independently ported and verified here.

## Current status

The new application has a working native shell, first-run profile store, built-in game catalogue, local installation detection, readiness assessment, profile editing, and native process launch. The complete parity position is tracked in [docs/PARITY_STATUS.md](docs/PARITY_STATUS.md).

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

Filesystem mutation, game detection, process launch, updates, backups, and publishing belong in Rust. React is an untrusted presentation layer and only receives narrow Tauri commands. Update sources must remain dedicated HTTPS/package locations; Discord invites are never downloads. Any future updater must retain MLLP's stage, verify, backup, apply, post-verify, and rollback contract.
