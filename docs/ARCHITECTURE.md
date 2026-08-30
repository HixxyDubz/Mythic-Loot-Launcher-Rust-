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
  -> local-first publishing services (Developer edition only)
Windows, filesystem, game launchers and GitHub CLI
```

React does not receive arbitrary shell or filesystem access. Native operations are exposed as purpose-specific commands from `src-tauri/src/lib.rs`. No API returns invented state outside Tauri; native absence is an error.

## Current Rust modules

- `models.rs`: server-free modpack profile/configuration contract, readiness types and twelve-game catalogue.
- `catalog.rs`: strict public catalogue schema, verified cache merge that preserves player-local state, and asynchronous GitHub catalogue/manifest refresh.
- `remote.rs`: bounded HTTPS metadata reads and rollback-capable atomic cache replacement.
- `storage.rs`: Tauri data-directory resolution, portable override, validation, corruption preservation, backup and staged replacement.
- `detection.rs`: configured candidates, Minecraft launcher/instance locations and Steam library/game scans.
- `readiness.rs`: fail-closed executable, folder, trusted-manifest and modpack-version gates.
- `manifest.rs`: bundled/local manifest loading, schema and path validation, and streaming SHA-256 verification.
- `minecraft_setup.rs`: deterministic minimal CurseForge ZIP and Modrinth `.mrpack` bootstrap generation from trusted Minecraft/loader metadata; no modpack files or personal state are embedded.
- `safe_path.rs`: traversal, Windows alias, alternate-stream and archive-member rejection.
- `publisher.rs` (Developer feature only): shell-free GitHub CLI preflight and fail-closed repository creation after explicit confirmation.
- `packager.rs` (Developer feature only): source-folder exclusions/privacy audit, deterministic ZIP and manifest generation, native release-plan caching, asset re-verification and explicitly confirmed immutable GitHub Release publication.
- `updater.rs`: trusted local/HTTPS and multipart package acquisition, archive validation, isolated changed-file staging, native preview caching, disk-space checks, pre-change ZIP backup, confirmed apply, post-verification and journaled rollback.
- `restore_points.rs`: hashed backup metadata, five-point retention, safe history listing, isolated restore staging, cached previews, explicit restore/delete confirmations, recovery-of-the-recovery backup, post-restore verification and rollback.
- `launch.rs`: validated native process start and Windows-aware argument parsing; it never generates server connection arguments.
- `safe_launch.rs`: manifest-scoped optional-file moves, launcher-owned recovery journals, exact child-process waiting, Windows PID liveness checks, hash-verified restoration and fail-closed restart recovery.
- `lib.rs`: narrow command boundary and main-window startup assertion.

## Current TypeScript modules

- `api.ts`: typed IPC facade that requires native Tauri for every operation and fails closed outside the desktop application.
- `App.tsx`: application orchestration and error/notice states.
- `editions/`: compile-time Player/Developer route selection. The Player module never imports the Publisher workspace.
- `components/`: title bar, modpack navigation, dashboard, settings, Developer publisher, Smart Launch, Safe Launch and staged update/repair workspaces.
- `types.ts`: IPC data contract mirrored from Rust.
- `test/fixtures.ts`: test-only component inputs imported exclusively by test files and excluded from both production bundles.

## Persistence

The native store resolves through Tauri's application data directory. Player and Developer use distinct application identifiers, so their normal installed state is separate and both editions can coexist. `MYTHIC_LOOT_DATA_DIR` overrides it only when explicitly set, which supports portable development and isolated acceptance runs. Invalid JSON is renamed to a timestamped `launcher-config.corrupt-*.json` before a fresh default is created.

Detected game roots and managed modpack roots are distinct. For 7 Days to Die, detection records the Steam game folder for launching and its `Mods` child as the installation/publishing root because generated manifest paths are relative to `Mods`. Minecraft uses a selected CurseForge or Modrinth instance root directly. The launcher records that launcher choice and routes its existing staged update/repair transaction into the selected instance; it does not copy an owner's live profile directly to another player. Saves, logs, screenshots, options, caches and launcher/account metadata are outside the trusted release inventory and remain untouched. Current and legacy Modrinth profile roots are detected. First-time setup creates deterministic launcher-native bootstrap archives under application data: a CurseForge import ZIP with `manifest.json`, or a Modrinth `.mrpack` with `modrinth.index.json`. Both declare the trusted Minecraft/loader versions and an empty file list so the launcher creates the profile before Mythic Loot performs the verified GitHub sync. Actual import in both third-party launchers remains an external acceptance gate.

The public catalogue feed is fixed to the launcher's GitHub Releases channel and contains only public profile identity, version, artwork, manifest, deployment and optional Discord metadata. It cannot carry executable paths, installation folders, launcher choices, launch arguments or installed versions. Startup renders immediately from bundled and last-verified cached state, then refreshes the bounded HTTPS catalogue and each dedicated manifest in a background command. Schema/path/URL/identity validation completes before atomic cache replacement; invalid or unavailable remote data leaves the previous verified cache and local player state in place. The modpack manifest's package URL and checksum take precedence over a legacy profile fallback so a refreshed manifest cannot accidentally download an older pinned package.

## Transaction boundary

Update preparation resolves the trusted manifest, downloads or copies the package into launcher-owned storage, rejects unsafe ZIP members, extracts only required changed files, and verifies the staged SHA-256 inventory before returning a preview. Apply accepts only that cached preview identifier plus explicit confirmation. It revalidates staged files, creates a backup of affected live paths, journals new paths, applies replacements/removals, verifies the complete live manifest, and restores overwritten, created, obsolete, and version-marker state if any apply or finalization step fails.

Every new backup contains a schema-versioned inventory of relative paths, sizes and SHA-256 values plus the update-created paths that must be removed to restore the earlier state. Recovery listing and deletion resolve only within the selected profile's launcher-owned backup folder. Restore preparation rejects archives without trustworthy metadata, extracts and verifies into isolated storage, and returns a cached preview. Confirmed restore creates a second backup of the current live state before mutation, verifies the restored inventory, and rolls itself back if mutation or final configuration persistence fails.

Safe Launch is a separate short-lived transaction. Rust resolves only `optionalFiles` from the trusted manifest, journals every source, disabled destination, size and SHA-256 before mutation, and stores the journal outside the live installation. It starts the configured game as a child, records that exact PID, waits for its exit and restores each unchanged disabled file. If the launcher exits early, a later run refuses recovery while that process is alive and offers explicit recovery afterward. Changed, missing, duplicated or redirected files fail closed and leave the journal for diagnosis.

GitHub publishing is a separate Developer workflow: local preparation scans privacy and produces reviewed artifacts without authentication; repository creation and release publication use authenticated `gh` state and separate explicit confirmations. Packages below 2 GiB remain one deterministic ZIP. Larger packages are split byte-for-byte into ordered 1 GiB assets, with each part and the complete ZIP represented by SHA-256 in the trusted manifest. The updater verifies each download, reconstructs the original ZIP, verifies its complete hash, then enters the existing stage/verify transaction. The native release command accepts only a cached preview identifier, not arbitrary asset paths, and re-hashes every cached part and the manifest at action time. Player builds compile without the `developer` Cargo feature, so `publisher.rs`, `packager.rs`, their Tauri command registrations, and the frontend Publisher route are absent rather than hidden by a runtime toggle.

Publisher privacy scanning excludes runtime state and non-runtime upstream README, changelog, licence and credits documents. This avoids packaging incidental author contact details or example machine paths while retaining runtime files and continuing to fail closed on credential-shaped runtime content.

## Explicit non-responsibility

No Rust command or React control may probe, configure, launch, stop, or join a game server. Server fields present in legacy Python manifests are ignored during deserialization for backward compatibility.
