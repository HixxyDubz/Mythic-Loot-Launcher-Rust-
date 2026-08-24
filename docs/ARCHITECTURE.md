# Architecture

## Process boundary

```text
React + TypeScript (untrusted presentation)
  -> narrow typed Tauri commands
Rust application core
  -> local configuration and recovery
  -> game/modpack detection and local readiness
  -> validated process launch
  -> future manifest/update/backup/publishing services
Windows, filesystem, game launchers and GitHub CLI
```

React does not receive arbitrary shell or filesystem access. Native operations are exposed as purpose-specific commands from `src-tauri/src/lib.rs`.

## Current Rust modules

- `models.rs`: server-free modpack profile/configuration contract, readiness types and twelve-game catalogue.
- `storage.rs`: Tauri data-directory resolution, portable override, validation, corruption preservation, backup and staged replacement.
- `detection.rs`: configured candidates, Minecraft launcher/instance locations and Steam library/game scans.
- `readiness.rs`: fail-closed executable, folder, trusted-manifest and modpack-version gates.
- `manifest.rs`: bundled/local manifest loading, schema and path validation, and streaming SHA-256 verification.
- `safe_path.rs`: traversal, Windows alias, alternate-stream and archive-member rejection.
- `publisher.rs`: shell-free GitHub CLI preflight and fail-closed repository creation after explicit confirmation.
- `launch.rs`: validated native process start and Windows-aware argument parsing; it never generates server connection arguments.
- `lib.rs`: narrow command boundary and main-window startup assertion.

## Current TypeScript modules

- `api.ts`: typed IPC facade with a browser-only design preview fallback.
- `App.tsx`: application orchestration and error/notice states.
- `components/`: title bar, modpack navigation, dashboard and settings.
- `types.ts`: IPC data contract mirrored from Rust.
- `mock.ts`: explicit browser-preview data; never used as native production persistence.

## Persistence

The native store resolves through Tauri's application data directory. `MYTHIC_LOOT_DATA_DIR` overrides it only when explicitly set, which supports portable development and isolated acceptance runs. Invalid JSON is renamed to a timestamped `launcher-config.corrupt-*.json` before a fresh default is created.

## Next backend sequence

The transactional updater builds on the trusted manifest layer, using stage/verify/backup/apply/post-verify/rollback. GitHub publishing is a separate Developer workflow: inspect authenticated `gh` state, preview repository/release mutations, generate privacy-safe artifacts, then require explicit confirmation before repository creation or release publication. Player builds only consume reviewed release metadata and never contain publishing controls.

## Explicit non-responsibility

No Rust command or React control may probe, configure, launch, stop, or join a game server. Server fields present in legacy Python manifests are ignored during deserialization for backward compatibility.
