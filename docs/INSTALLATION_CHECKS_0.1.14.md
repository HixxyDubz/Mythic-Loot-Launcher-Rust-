# Local installation checks: 0.1.14

Implementation started 2026-09-21; final verification 2026-09-25. Rust only. Python/MLLP is read-only reference. No server feature, live game/modpack mutation, or public app/feed publication is included.

## Using the checks

Both editions now offer **Settings → Check local game installation** for non-Minecraft profiles. Minecraft retains its separate version/loader metadata inspector. The check uses the paths currently shown in the form without saving them, starting the selected executable, creating folders or contacting any server. Changing a path/profile/game discards earlier and pending results. Unsaved changes to the game type must be saved before native inspection; the backend resolves the saved profile/game and published requirements itself.

The result distinguishes missing, unconfigured, present and unsafe/unreadable paths, recognises known client layouts, warns when a selected executable differs from those clients, and shows metadata evidence separately from compatibility. Custom/manual clients are not rejected merely because their layout is unknown. Missing modpack folders are reported without being created. This is advisory; it does not change launch readiness, installed-modpack versions, sync targets or update decisions, and it does not prove binary integrity or gameplay compatibility.

## Detection fixes

- Current configuration now carries its game root separately from its exact managed modpack folder. Selecting it preserves a custom target instead of nesting deployment subfolders such as `Mods/Mods`.
- When no game root was configured, recognised nested executable suffixes can resolve the root (for example Factorio `bin/x64` and Palworld's nested Win64 client). The modpack target is never assumed to be the game root.
- Configured CurseForge/Modrinth selections preserve their launcher type and instance directory.
- Selecting a detected installation with no executable clears the previous launch executable in the unsaved settings draft rather than accidentally launching the old game. Saving still requires the normal explicit action.
- Restored legacy Marvel Heroes `MHGame.exe` layouts and Factorio's root-level client candidate. Existing client candidates remain accepted.
- Added the Python reference's common alternate Steam roots on drives C through H. Steam library metadata reads are size-bounded; Minecraft child-directory enumeration is capped at 512 entries per launcher root and rejects redirected paths. Manual selection remains available outside discovery coverage.

## Version evidence and limits

For 7DTD, Palworld, Core Keeper, Valheim, Factorio and Stardew Valley, a Steam installation in `steamapps/common/<folder>` may supply an installed build ID from its known app manifest. The parser verifies the AppState application ID, exact selected folder, fully-installed/idle state and numeric build ID. Duplicate/malformed fields and conflicting identities are rejected. Nested depot fields cannot override top-level values. Only allowlisted identity/build fields are extracted; account-owner data never crosses IPC.

Steam build IDs are **not** semantic game versions and are never compared to a requirement such as `3.1`. The owner's 7DTD EXE reports Unity's `2022.3.62f2` resource version, while its managed assembly reports `0.0.0.0`; neither is treated as the game version. 7DTD remains explicitly unknown at the semantic-version level until a reliable reader is available.

Factorio reads the declared version from `data/base/info.json`, requiring `name: base` and a numeric three-component version. Only an exact, likewise supported published requirement is compared; partial/range/unsupported requirements remain unknown. This is metadata declaration, not verification of the running game. The [official Factorio mod-structure documentation](https://lua-api.factorio.com/latest/auxiliary/mod-structure.html) describes `info.json` identity/version fields, and its [file-path documentation](https://lua-api.factorio.com/latest/types/FileName.html) identifies the base data directory. Steam application identities were checked against the official [Palworld](https://store.steampowered.com/app/1623730/Palworld/), [Core Keeper](https://store.steampowered.com/app/1621690/Core_Keeper/), [Valheim](https://store.steampowered.com/app/892970/Valheim/), [Factorio](https://store.steampowered.com/app/427520/Factorio/) and [Stardew Valley](https://store.steampowered.com/app/413150-Stardew-Valley/?l=english) pages and the local 7DTD manifest.

Metadata reads are limited to regular non-linked files of at most 1 MiB. Steam parsing has token/depth limits and generic errors that do not echo raw data. Unsupported games/versions, missing exports or custom installations stay manual/unknown. Hytale discovery and Factorio custom/default deployment routing are still separate remaining work; this change does not silently choose a Factorio user-data location.

## Verification

- Controlled native tests cover root/target separation, nested executable roots, preserving Minecraft launcher identity, Steam-vs-game version distinction, owner-data projection, duplicate/nested/malformed app metadata, wrong identity/folder/state refusal, Factorio exact comparison and missing/oversized metadata.
- Frontend tests cover on-demand inspection, stale-path results, failures after prior success, unsafe paths, unknown/mismatch results and explicit configured-target selection without saving automatically.
- Final frontend suite: 76 tests across 21 files passed. Developer Rust: 159 passed, six intentionally ignored; Player Rust: 119 passed, five intentionally ignored. The explicit 7DTD read-only test below was run separately. Previously gated long-transfer/live-package tests were not rerun.
- Strict Clippy passed both editions with `--all-targets -- -D warnings`. TypeScript type-checking, locked Cargo metadata validation and source diff whitespace checks passed.
- Explicit read-only acceptance against the owner's 7DTD installation found its client and Steam build **24994517**. Its Steam manifest SHA-256 was identical before/after: `86B0F781A9DA539A79840C9C800563A8A2372EBB940BD3BAB033806B7AA1B04A`. The game was not launched. A build ID does not prove its current semantic game version.

- Both 0.1.14 NSIS installers and win-unpacked EXEs built successfully. Production-mock source/dist scans and Player edition-separation checks passed. All four artifact sizes and SHA-256 values were independently verified against `artifacts/windows/build-manifest.json`.
- The packaged Player updater helper passed replacement/backup/hash/restart, readiness while the parent remained alive, and corrupt-stage rejection checks in disposable local folders. This was not a live GitHub update transition.
- Both packaged editions passed isolated responsive-window/native-startup smoke checks and preserved preferences across restart without starting disabled catalogue/app update checks. This does not replace visual review or clean-machine installer acceptance.

### Built artifacts

Paths are relative to `artifacts/windows`. Binaries are local outputs, not committed source files.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.14.exe` | 5,459,363 | `7E602EB2CCA15EAFF6BC3599092CD92A6EAB82DC7944C99BF49056F8DFDA0FC6` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,670,080 | `612AED6B38A23EE09C8CDE8CF9F7AC3767A5CC209D6B04A32B23F7FF14F918C7` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.14.exe` | 5,698,806 | `D98DB3BCFEAD4306DB4EFBFABA34A6358F9452B46D229947445D93BE224BC45D` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,725,312 | `E4302974295A054903C26F88FB3AC4089F3ACC531CCFCDA7E1F575F274ED8A64` |

Real clean-machine, launcher-import and gameplay checks remain external acceptance gates. This checkpoint is not feature completeness or a zero-bug guarantee. A source push does not publish an app release or change the public update feed.
