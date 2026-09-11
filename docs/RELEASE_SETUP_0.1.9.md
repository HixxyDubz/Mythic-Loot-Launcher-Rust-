# Release setup checkpoint: 0.1.9

Implementation: 2026-09-10; final packaged checks: 2026-09-11. Rust only; Python and live game/modpack folders were not modified. This is a tested development checkpoint, not a claim of complete parity or a public release.

## Using the 7 Days to Die publisher

1. Open Developer, select the 7DTD modpack, then Publisher.
2. In Local release preparation, browse to the **Mods folder containing the files you edit**, not the entire game installation. This source can change between releases and is independent of Player's destination folder.
3. Choose a saved **Game version for this release**, or type a new one. Enter the separate **Modpack version**, release date, repository and notes.
4. Save publishing choices locally, or prepare the release (successful preparation also remembers them). Neither action uploads files. Source-folder choices and game-version history stay on this computer.
5. Review the exact game version, source, inventory, package hashes and privacy results. Publication still requires GitHub authentication and explicit confirmation.
6. After publishing the pack, prepare and publish the public catalogue so Player receives the updated profile metadata as well as the latest manifest. Saving settings or pushing launcher source alone does not publish a modpack.

The manifest uses the reviewed game version, not a permanent 7DTD default. Successful publication updates the local profile to that same version for the next catalogue publication. No fixed list of supposedly current 7DTD releases is fabricated.

## Other fixes

- Native folder browsing for source, game directory and modpack destination; executable selection for the game/launcher. Cancelling leaves the existing path unchanged. Rust accepts only existing local paths of the requested type and rejects redirected paths.
- Minecraft publication requires an explicit supported loader identity. Arbitrary profiles can author NeoForge, Forge, Fabric or Quilt metadata, and both import archive formats use it. The Settings guidance no longer incorrectly assumes every pack uses NeoForge. This validates identity syntax, not the real-world compatibility of every version/loader combination.
- Newly saved News, Rules and Changelog are presentation-only local drafts. A remote refresh can update package/version/inventory metadata without replacing these drafts. Invalid saved drafts are surfaced rather than silently discarded. Old-style pre-upgrade drafts embedded in downloaded manifests are not automatically distinguished or migrated yet.
- Existing optional-file classifications survive repackaging; removed optional paths are included in the reviewed diff. Player extras authoring/management is still unfinished.
- Player reconciles catalogue removals as archived entries, preserves local paths and installed-version state, and exposes an archived list for accessing retained installations. Republishing the same ID restores visibility. Automatic refresh skips archived Player manifests.
- HTTPS catalogue artwork and news banners are permitted by the webview image policy; remote images omit referrers. Other webview network/script restrictions are unchanged. This is not a claim of visual validation against a particular remote image.
- Preview identity includes all reviewed release choices, profile metadata and the base manifest. Changing versions, source, notes or saved content requires a new review. Preview workspaces cannot overlap the source or follow redirected directories.
- New publishing IPC and persistence remain Developer-only. The packaging edition-separation scan now checks the new command names too.

## Verification

- Frontend: 36 tests across 15 files passed; TypeScript and both edition production builds passed.
- Developer Rust: 117 passed, four explicitly ignored. Player Rust: 86 passed, three explicitly ignored. The ignored cases are the long transfer, real Minecraft package, Developer-only live 7DTD source, and subprocess-only lock child (the parent regression invokes that child separately). The large live-package checks were not rerun for this checkpoint.
- Both editions passed Clippy with `--all-targets -- -D warnings`.
- Source/dist no-production-mock-data scans and the Player/Developer separation scan passed during packaging.
- Both final win-unpacked EXEs created responsive windows, correct edition titles and isolated native configuration/activity files. Temporary test profiles were removed by the guarded smoke script; real profiles were untouched.
- The final packaged Player helper activated different reviewed bytes under the public hyphenated EXE name, retained the exact backup, verified SHA-256, recorded success and restarted the probe. Separate packaged checks passed readiness while the parent remained alive and corrupt-stage rejection without changing the target.
- Both NSIS installers were built and hash-verified, but were not installed on a clean machine. Native Browse interaction and visual remote-artwork checks remain human checks; component tests cover the path-selection response and cancellation behavior.

Test fixtures exist only in test code; no production fallback/mock data was added.

### Artifacts

Paths below are relative to `artifacts/windows`. All four lengths and SHA-256 hashes were re-read from disk and matched `build-manifest.json`.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.9.exe` | 5,362,607 | `039BA867327BC51BD205DB58F9B7409CFD33C685ECFA137497FA3CC305F04716` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,213,376 | `19D9A06F13D6FC20F2958B35E9F76FA778E19790C2C2E7F2F9D376330D9F6C0F` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.9.exe` | 5,551,176 | `96A63CF6D5D3FD4BAEC7D648C16AF8EC1C7901964D9C2562CDB1348529201F7E` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,208,704 | `6779BA9E7263757B6F2A8240E46534C4B0D8C633285035BAC5998FEC3E7ADB5F` |

## Not included in this checkpoint

The feature-completion checklist remains authoritative for unfinished runtime/preferences, extras, crash/recovery and progress/cancellation work. Real CurseForge/Modrinth imports, clean-machine installer execution, actual gameplay and a reviewed 7DTD publication are separate acceptance gates.

The public GitHub release was checked during this work and is still **v0.1.7**. This source update and local build do not publish an app-update feed. Public app publication/live upgrade acceptance still has to happen before calling these new fixes distributed to existing players.
