# Architecture

## Process boundary

```text
React + TypeScript (untrusted presentation)
  -> narrow typed Tauri commands
Rust application core
  -> local configuration and recovery
  -> game/modpack detection and local readiness
  -> validated process launch
  -> persistent optional-file isolation and crash recovery
  -> manifest verification and transactional update/repair services
  -> metadata-backed restore-point recovery
  -> local-first publishing services
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
- `packager.rs`: source-folder exclusions/privacy audit, deterministic ZIP and manifest generation, native release-plan caching, asset re-verification and explicitly confirmed immutable GitHub Release publication.
- `updater.rs`: trusted local/HTTPS and multipart package acquisition, archive validation, isolated changed-file staging, native preview caching, disk-space checks, pre-change ZIP backup, confirmed apply, post-verification and journaled rollback.
- `restore_points.rs`: hashed backup metadata, five-point retention, safe history listing, isolated restore staging, cached previews, explicit restore/delete confirmations, recovery-of-the-recovery backup, post-restore verification and rollback.
- `launch.rs`: validated native process start and Windows-aware argument parsing; it never generates server connection arguments.
- `safe_launch.rs`: manifest-scoped optional-file moves, launcher-owned recovery journals, exact child-process waiting, Windows PID liveness checks, hash-verified restoration and fail-closed restart recovery.
- `lib.rs`: narrow command boundary and main-window startup assertion.

## Current TypeScript modules

- `api.ts`: typed IPC facade with a browser-only design preview fallback.
- `App.tsx`: application orchestration and error/notice states.
- `components/`: title bar, modpack navigation, dashboard, settings, publisher, and staged update/repair workspace.
- `types.ts`: IPC data contract mirrored from Rust.
- `mock.ts`: explicit browser-preview data; never used as native production persistence.

## Persistence

The native store resolves through Tauri's application data directory. `MYTHIC_LOOT_DATA_DIR` overrides it only when explicitly set, which supports portable development and isolated acceptance runs. Invalid JSON is renamed to a timestamped `launcher-config.corrupt-*.json` before a fresh default is created.

Detected game roots and managed modpack roots are distinct. For 7 Days to Die, detection records the Steam game folder for launching and its `Mods` child as the installation/publishing root because generated manifest paths are relative to `Mods`. Minecraft currently has no deployment subdirectory and therefore uses its detected instance root directly.

## Transaction boundary

Update preparation resolves the trusted manifest, downloads or copies the package into launcher-owned storage, rejects unsafe ZIP members, extracts only required changed files, and verifies the staged SHA-256 inventory before returning a preview. Apply accepts only that cached preview identifier plus explicit confirmation. It revalidates staged files, creates a backup of affected live paths, journals new paths, applies replacements/removals, verifies the complete live manifest, and restores overwritten, created, obsolete, and version-marker state if any apply or finalization step fails.

Every new backup contains a schema-versioned inventory of relative paths, sizes and SHA-256 values plus the update-created paths that must be removed to restore the earlier state. Recovery listing and deletion resolve only within the selected profile's launcher-owned backup folder. Restore preparation rejects archives without trustworthy metadata, extracts and verifies into isolated storage, and returns a cached preview. Confirmed restore creates a second backup of the current live state before mutation, verifies the restored inventory, and rolls itself back if mutation or final configuration persistence fails.

Safe Launch is a separate short-lived transaction. Rust resolves only `optionalFiles` from the trusted manifest, journals every source, disabled destination, size and SHA-256 before mutation, and stores the journal outside the live installation. It starts the configured game as a child, records that exact PID, waits for its exit and restores each unchanged disabled file. If the launcher exits early, a later run refuses recovery while that process is alive and offers explicit recovery afterward. Changed, missing, duplicated or redirected files fail closed and leave the journal for diagnosis.

GitHub publishing is a separate Developer workflow: local preparation scans privacy and produces reviewed artifacts without authentication; repository creation and release publication use authenticated `gh` state and separate explicit confirmations. The native release command accepts only a cached preview identifier, not arbitrary asset paths, and re-hashes the cached assets at action time. Player builds will only consume reviewed release metadata and must not contain publishing controls.

## Explicit non-responsibility

No Rust command or React control may probe, configure, launch, stop, or join a game server. Server fields present in legacy Python manifests are ignored during deserialization for backward compatibility.
