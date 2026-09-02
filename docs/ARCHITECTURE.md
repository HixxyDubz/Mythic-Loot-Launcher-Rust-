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
- `catalog_publisher.rs` (Developer feature only): public-only catalogue projection, native preview caching, hash revalidation and explicitly confirmed GitHub Contents publication.
- `content_editor.rs` (Developer feature only): bounded News/Rules/Changelog editing that preserves distribution metadata, validates the complete candidate manifest and atomically saves only under launcher-owned data.
- `content_publisher.rs` (Developer feature only): manifest-only release staging, immutable package-reference checks, native preview caching, asset revalidation and explicitly confirmed latest-release publication.
- `app_update_publisher.rs` (Developer feature only): build-manifest validation, independent Player artifact re-hashing, fixed checksum-feed generation, exact release-plan caching and explicitly confirmed version-tagged launcher release publication.
- `remote.rs`: bounded HTTPS metadata reads and rollback-capable atomic cache replacement.
- `activity.rs`: persistent native-operation history, process-interruption recovery, bounded retention, corruption preservation and finished-item cleanup.
- `storage.rs`: Tauri data-directory resolution, portable override, validation, corruption preservation, backup and staged replacement.
- `storage_maintenance.rs`: bounded read-only usage inventory plus fixed-target, explicitly confirmed cache, old-backup and aged temporary-work cleanup that refuses links and reparse points.
- `support.rs`: known-location game-log discovery, bounded tail extraction, personal/secret/network redaction, exact in-memory review caching and explicitly confirmed fixed-destination ZIP export.
- `detection.rs`: configured candidates, Minecraft launcher/instance locations and Steam library/game scans.
- `readiness.rs`: fail-closed executable, folder, trusted-manifest and modpack-version gates.
- `manifest.rs`: bundled/local manifest loading, typed News/Rules/Changelog projection, schema and path validation, and streaming SHA-256 verification.
- `minecraft_setup.rs`: deterministic minimal CurseForge ZIP and Modrinth `.mrpack` bootstrap generation from trusted Minecraft/loader metadata; no modpack files or personal state are embedded.
- `safe_path.rs`: traversal, Windows alias, alternate-stream and archive-member rejection.
- `publisher.rs` (Developer feature only): shell-free GitHub CLI preflight and fail-closed repository creation after explicit confirmation.
- `packager.rs` (Developer feature only): source-folder exclusions/privacy audit, deterministic ZIP and manifest generation, native release-plan caching, asset re-verification and explicitly confirmed immutable GitHub Release publication.
- `updater.rs`: trusted local/HTTPS and multipart package acquisition, archive validation, isolated changed-file staging, native preview caching, disk-space checks, pre-change ZIP backup, confirmed apply, post-verification and journaled rollback.
- `restore_points.rs`: hashed backup metadata, five-point retention, safe history listing, isolated restore staging, cached previews, explicit restore/delete confirmations, recovery-of-the-recovery backup, post-restore verification and rollback.
- `launch.rs`: validated native process start and Windows-aware argument parsing; it never generates server connection arguments.
- `safe_launch.rs`: manifest-scoped optional-file moves, launcher-owned recovery journals, exact child-process waiting, Windows PID liveness checks, hash-verified restoration and fail-closed restart recovery.
- `self_update.rs`: fixed Player release feed validation, semantic version gates, bounded size/hash-verified executable staging, isolated post-exit replacement, verified backup/rollback, result journaling and restart.
- `lib.rs`: narrow command boundary and main-window startup assertion.

## Current TypeScript modules

- `api.ts`: typed IPC facade that requires native Tauri for every operation and fails closed outside the desktop application.
- `App.tsx`: application orchestration and error/notice states.
- `editions/`: compile-time Player/Developer route selection. The Player module never imports the Publisher workspace.
- `components/`: title bar, modpack navigation, dashboard, Activity Centre, Storage, Support, settings, Developer publishers, app update, Smart Launch, Safe Launch and staged modpack update/repair workspaces.
- `types.ts`: IPC data contract mirrored from Rust.
- `test/fixtures.ts`: test-only component inputs imported exclusively by test files and excluded from both production bundles.

## Persistence

The native store resolves through Tauri's application data directory. Player and Developer use distinct application identifiers, so their normal installed state is separate and both editions can coexist. `MYTHIC_LOOT_DATA_DIR` overrides it only when explicitly set, which supports portable development and isolated acceptance runs. Invalid JSON is renamed to a timestamped `launcher-config.corrupt-*.json` before a fresh default is created.

Native catalogue, storage, support, setup, verification, update/repair, restore, launch and Developer publishing operations write a schema-versioned `activity-history.json` beside the edition's configuration. The store is atomically replaced, bounded to 4 MiB and the newest 100 records, while the UI exposes only the newest 12. Finished records can be cleared without touching active work. An unfinished record from an earlier launcher process is marked failed and interrupted on the next read rather than remaining permanently active; malformed history is preserved as `activity-history.corrupt-*.json`. Activity recording is observational: a history-write failure is logged but never changes the authority or result of the real operation.

Storage inventory measures the complete launcher data root and each configured modpack folder without following symbolic links, junctions or other Windows reparse points; scanning is bounded to one million entries and reports incomplete access instead of silently assuming zero. Configured modpack folders are read-only inventory targets and can never be submitted for cleanup. React can request only three native cleanup enum values, never a filesystem path: retain the newest five ZIP restore points inside each known profile backup folder, clear contents of the fixed verified-catalogue cache, or remove direct launcher staging/Developer-preview entries older than 24 hours. Every cleanup requires explicit confirmation, refuses linked roots and entries, leaves the category root in place, blocks while another native activity is running and returns a newly measured report.

Support review searches only game-specific known log folders. Minecraft uses the configured instance's direct `logs/latest.log` or crash reports; 7DTD uses direct log files under its normal application-data and configured game roots. Candidate folders and files must be regular, non-linked entries, each directory scan is bounded to 10,000 entries and source logs above 64 MiB are excluded. Rust keeps at most the newest 500 lines and 512 KiB, replaces environment-derived home/user identity plus obvious credentials, tokens, URL credentials, email, IPv4/IPv6 and hardware addresses, then returns the exact summary and excerpt for review. Export accepts only that cached preview identifier and confirmation, writes atomically into launcher-owned `support-bundles`, and can contain only `summary.json`, `summary.txt` and the reviewed redacted excerpt. Server configuration, game options and full configuration files are never included.

Detected game roots and managed modpack roots are distinct. For 7 Days to Die, detection records the Steam game folder for launching and its `Mods` child as the installation/publishing root because generated manifest paths are relative to `Mods`. Minecraft uses a selected CurseForge or Modrinth instance root directly. The launcher records that launcher choice and routes its existing staged update/repair transaction into the selected instance; it does not copy an owner's live profile directly to another player. Saves, logs, screenshots, options, caches and launcher/account metadata are outside the trusted release inventory and remain untouched. Current and legacy Modrinth profile roots are detected. First-time setup creates deterministic launcher-native bootstrap archives under application data: a CurseForge import ZIP with `manifest.json`, or a Modrinth `.mrpack` with `modrinth.index.json`. Both declare the trusted Minecraft/loader versions and an empty file list so the launcher creates the profile before Mythic Loot performs the verified GitHub sync. Actual import in both third-party launchers remains an external acceptance gate.

The public catalogue feed is fixed to `launcher-catalog.json` on the launcher's public `main` branch and contains only public profile identity, version, artwork, manifest, deployment and optional Discord metadata. It cannot carry executable paths, installation folders, launcher choices, launch arguments or installed versions. Player startup renders immediately from bundled and last-verified cached state, then refreshes the bounded HTTPS catalogue and each dedicated manifest in a background command. Schema/path/URL/identity validation completes before atomic cache replacement; invalid or unavailable remote data leaves the previous verified cache and local player state in place. Developer never merges the remote catalogue back over its authoring state. New profiles are hidden drafts; release publication records the latest manifest URL and enables visibility, while the Developer may disable visibility to archive a pack from the next catalogue. The modpack manifest's package URL and checksum take precedence over a legacy profile fallback so a refreshed manifest cannot accidentally download an older pinned package.

## Transaction boundary

Update preparation resolves the trusted manifest, downloads or copies the package into launcher-owned storage, rejects unsafe ZIP members, extracts only required changed files, and verifies the staged SHA-256 inventory before returning a preview. Apply accepts only that cached preview identifier plus explicit confirmation. It revalidates staged files, creates a backup of affected live paths, journals new paths, applies replacements/removals, verifies the complete live manifest, and restores overwritten, created, obsolete, and version-marker state if any apply or finalization step fails.

Every new backup contains a schema-versioned inventory of relative paths, sizes and SHA-256 values plus the update-created paths that must be removed to restore the earlier state. Recovery listing and deletion resolve only within the selected profile's launcher-owned backup folder. Restore preparation rejects archives without trustworthy metadata, extracts and verifies into isolated storage, and returns a cached preview. Confirmed restore creates a second backup of the current live state before mutation, verifies the restored inventory, and rolls itself back if mutation or final configuration persistence fails.

Safe Launch is a separate short-lived transaction. Rust resolves only `optionalFiles` from the trusted manifest, journals every source, disabled destination, size and SHA-256 before mutation, and stores the journal outside the live installation. It starts the configured game as a child, records that exact PID, waits for its exit and restores each unchanged disabled file. If the launcher exits early, a later run refuses recovery while that process is alive and offers explicit recovery afterward. Changed, missing, duplicated or redirected files fail closed and leave the journal for diagnosis.

GitHub publishing is a separate Developer workflow: local preparation scans privacy and produces reviewed artifacts without authentication; repository creation, release publication, content-only release publication and public-catalogue replacement use authenticated `gh` state and separate explicit confirmations. The Developer content editor accepts a profile id and typed presentation fields, clones the existing trusted manifest (or creates a real profile-bound draft before the first release), changes no package metadata, validates the complete candidate and atomically saves it. Content-only preparation derives its repository and asset name from the profile's fixed GitHub latest-manifest URL, requires a current published package version, HTTPS package assets and complete hashes, then copies exactly one reviewed manifest into native staging. Confirmed publication creates an immutable content tag, uploads no package ZIP and marks that manifest-only release latest; the manifest continues to reference the earlier immutable package release assets. Packages below 2 GiB remain one deterministic ZIP. Larger packages are split byte-for-byte into ordered 1 GiB assets, with each part and the complete ZIP represented by SHA-256 in the trusted manifest. The updater verifies each download, reconstructs the original ZIP, verifies its complete hash, then enters the existing stage/verify transaction. A successful package release also activates its reviewed manifest in local trusted storage before subsequent content editing. Native release and catalogue commands accept only cached preview identifiers, not arbitrary frontend paths, and re-hash every reviewed artifact at action time. Catalogue publication targets only the fixed launcher repository, branch and filename through GitHub's Contents API; it never accepts a token or arbitrary destination from React. Player builds compile without the `developer` Cargo feature, so every Developer publisher/editor module, command registration and frontend route—including app-release publication—is absent rather than hidden by a runtime toggle.

Application self-update is a separate Player-only transaction. Rust reads only the fixed latest-release feed from the launcher repository, validates its strict identity, semantic versions, fixed Player asset URL, byte count and SHA-256, then caches the reviewed feed rather than accepting a URL from React. Download writes into the fixed `app-update-staging` root, enforces both declared and hard size limits, checks the PE header and exact hash, and caches the resulting native stage. Confirmed apply copies the currently running Player executable as an isolated helper and exits the UI process. The helper waits for that exact PID, re-verifies current/staged bytes, retains an exact backup, stages an adjacent replacement, activates by rename, verifies the installed hash and verifies any rollback before restarting Player. Developer can check the public feed but can never apply it. Its separate app-release publisher accepts a Windows build manifest path only for local review, verifies product/version and independently hashes the Player portable and installer, then stages those two fixed names plus the generated feed. Confirmed publication always targets the fixed launcher repository and a previously unused version tag; no token, repository, arbitrary asset or Developer executable comes from React. GitHub repository-level release immutability is a separate hosting setting and is not assumed by the application.

Publisher privacy scanning excludes runtime state and non-runtime upstream README, changelog, licence and credits documents. This avoids packaging incidental author contact details or example machine paths while retaining runtime files and continuing to fail closed on credential-shaped runtime content.

## Explicit non-responsibility

No Rust command or React control may probe, configure, launch, stop, or join a game server. Server fields present in legacy Python manifests are ignored during deserialization for backward compatibility.
