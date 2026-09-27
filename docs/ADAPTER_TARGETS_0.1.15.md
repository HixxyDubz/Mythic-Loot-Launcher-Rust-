# Adapter targets and dependency security: 0.1.15

Implementation and dependency audit: 2026-09-27. Final packaged acceptance: 2026-09-28. Rust only; Python/MLLP is read-only reference. No server functionality, live game-folder mutation, signing-key creation or public feed publication.

## Factorio

Settings detection searches Steam, standard Program Files installations and the configured game directory/recognised executable root. It reads bounded local configuration to propose the actual `mods` folder, independently of the game binary location. It supports `config-path.cfg` → `config.ini` → `[path] write-data`, absolute paths and the executable/system-write-data macros. Supported absolute `--config`/`-c` and `--mod-directory` arguments in Mythic Loot's settings take precedence. Executable-path overrides remain manual.

The declared write-data path is required: bootstrap default-generation flags alone do not prove an active target. Missing files, duplicate keys, unsupported macros, relative paths, ambiguous comment markers and unsafe paths produce an explicit unresolved result. Selecting it leaves the modpack target blank instead of inventing a game-root destination. UNC/device paths, volume roots and redirected paths are not automatically suggested. Reads are limited to 1 MiB and never run the executable or create a mods directory. Steam/external shortcut launch arguments are not inspected; users must review the target.

Current configured targets remain separate candidates, even if discovery proposes a different target for the same game root. Selecting any candidate only edits the unsaved Settings form. Editing discovery inputs hides stale candidates; failed re-detection cannot leave old results selectable.

Basis: [official application-directory documentation](https://wiki.factorio.com/User_data_directory) and [command-line parameters](https://wiki.factorio.com/Command_line_parameters). No claim is made that every Factorio configuration syntax or third-party wrapper is supported.

## Hytale

Within the existing `%APPDATA%/Hytale` root, discovery pairs `UserData/Mods` with the installed release patchline. Existing non-release data directories under `data/<patchline>` are paired only with their corresponding installed patchline. Enumeration is bounded to 32 entries and redirected paths are rejected. Missing launchers require manual executable selection; no server executable is selected. Custom roots remain manual.

Users must select the same patchline in Hytale Launcher themselves. No launcher settings, account state or patchline selection is changed. Guidance points to Hytale's own User Data folder button. The [official Update 5 notes](https://hytale.com/news/2026/5/update-5-patch-notes) explain separated patchline data; the [current support guide](https://support.hytale.com/hc/en-us/articles/45315447521947-How-to-Find-Your-Hytale-Logs) documents default/custom roots and the launcher folder button.

## Security audit

- Installed cargo-audit 0.22.2 with the owner's approval.
- Initial audit found rustls 0.23.43 affected by [RUSTSEC-2026-0285 / GHSA-2mjx-qc3c-rqvc](https://github.com/rustls/rustls/security/advisories/GHSA-2mjx-qc3c-rqvc). Updated only that dependency to patched 0.23.45; no broad dependency upgrades.
- Re-audit on September 27 reports zero entries in the known-vulnerability list, with **seven warnings still present** in the full cross-platform lockfile: unmaintained `proc-macro-error`, `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`, `unic-ucd-version`, and unsound `glib`.
- Windows-target dependency inspection finds no `glib` or `proc-macro-error` dependency tree. The UNIC crates remain transitively used by Tauri's URL-pattern utilities. These warnings are not suppressed; Linux GTK dependency remediation remains upstream/platform work before any Linux distribution.
- Windows code signing remains deliberately disabled. Cryptographic update signing is a different mechanism and can use locally generated keys without a Windows publisher certificate; see [Tauri signing documentation](https://v2.tauri.app/plugin/updater/#signing-updates). This checkpoint does not change the current unsigned, HTTPS/checksum-based update protocol.

## Verification and limits

- Frontend: 78 tests across 21 files passed; TypeScript type-check passed.
- Rust: Developer 169 passed, six intentionally ignored; Player 129 passed, five intentionally ignored. The ignored real-folder/large-package/long-transfer cases were not rerun for this checkpoint.
- Both editions passed strict Clippy with `--all-targets -- -D warnings`.
- npm audit: zero known vulnerabilities. Locked Cargo metadata resolves the launcher to 0.1.15; version fields are synchronized.
- New regressions cover declared system/custom/portable Factorio data, command-line precedence, missing/oversized/malformed metadata, unsafe paths, distinct Hytale patchlines, no directory creation, duplicate-root target preservation, and frontend blank-target/stale-result behavior.
- Both 0.1.15 NSIS installers and win-unpacked EXEs built successfully; source/dist no-production-mock checks and Player edition-separation checks passed.
- Both packaged editions passed isolated responsive-window/native-activity startup and preference persistence checks, including respecting disabled startup update checks. This was not a visual review of every screen or a clean-machine installer test.
- The packaged Player update helper passed different-byte replacement, exact backup preservation, hash verification and restart; separate checks verified readiness while the parent remained alive and corrupt-stage rejection without changing the target. This was not a live GitHub old-to-new upgrade.

### Verified artifacts

All four sizes and hashes were independently re-read and matched `artifacts/windows/build-manifest.json`. Paths below are relative to `artifacts/windows`; binaries are local outputs, not committed source files.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.15.exe` | 5,472,285 | `2E35B55581D4E0131F2CC712BDE82720DFAF19E031F82A0FE9DA8B5F6DF1C49F` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,733,056 | `40E821AC29C0F7CAF8364CF8CBD257D585F2080DCC73D13825350E07FD577044` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.15.exe` | 5,685,439 | `F68B2250B2832FE56AF20F648CEA7C61DAC225BE81DDE0DE4029648A5FB1F300` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,710,976 | `D3E7D77362E651D4C0462CCC3CBA0A21F792A8BFD5788E34F2557972210FD471` |

Test fixtures are disposable and not production data. Real Factorio/Hytale launch acceptance, clean-machine installers, live GitHub upgrade transitions, additional preferences, crash prompts, native progress/cancellation and hard-interruption transaction recovery are not completed by this checkpoint.
