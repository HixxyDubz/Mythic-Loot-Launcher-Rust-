# Architecture

## Process boundary

```text
React + TypeScript (untrusted presentation)
  -> narrow typed Tauri commands
Rust application core
  -> local configuration and recovery
  -> game detection and readiness
  -> validated process launch
  -> future manifest/update/backup/publishing services
Windows, filesystem, game launchers and external services
```

React does not receive arbitrary shell or filesystem access. Native operations are exposed as purpose-specific commands from `src-tauri/src/lib.rs`.

## Current Rust modules

- `models.rs`: serialized profile/configuration contract, readiness types and twelve-game catalogue.
- `storage.rs`: Tauri data-directory resolution, portable override, validation, corruption preservation, backup and staged replacement.
- `detection.rs`: configured candidates, Minecraft launcher/instance locations and Steam library/game scans.
- `readiness.rs`: fail-closed executable, folder and modpack-version gates.
- `launch.rs`: validated native process start, Windows-aware argument parsing and supported direct-join arguments.
- `lib.rs`: narrow command boundary and main-window startup assertion.

## Current TypeScript modules

- `api.ts`: typed IPC facade with a browser-only design preview fallback.
- `App.tsx`: application orchestration and error/notice states.
- `components/`: title bar, server navigation, dashboard and settings.
- `types.ts`: IPC data contract mirrored from Rust.
- `mock.ts`: explicit browser-preview data; never used as native production persistence.

## Persistence

The native store resolves through Tauri's application data directory. `MYTHIC_LOOT_DATA_DIR` overrides it only when explicitly set, which supports portable development and isolated acceptance runs. Invalid JSON is renamed to a timestamped `launcher-config.corrupt-*.json` before a fresh default is created.

## Next backend sequence

Manifest schema/path validation must precede the server protocols and Smart Play port. The transactional updater then builds on that trusted manifest layer. Backups, restore points and Safe Launch follow the same mutation journal. Owner publishing and self-update remain last because they depend on every earlier safety boundary.
