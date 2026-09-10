# Rust launcher 0.1.8 blocker-fix checkpoint

Date: 2026-09-10. Scope: the five high-priority adversarial findings F01-F05. Python/MLLP is retired and was not modified. Live game directories are not test fixtures.

## Fixes

| Finding | Change | Safety boundary |
|---|---|---|
| F01: GitHub portable name rejected by replacement helper | Accept the exact hyphenated release name, spaced installed name and native build name; probe installation-folder write access; validate current/staged bytes before acknowledging readiness; parent waits for the exact journal hash and parent PID before closing. | Early helper failure leaves Player open. Arbitrary names, Developer binaries and installers remain rejected. Older binaries still contain their old helper. |
| F02: one-minute total package download deadline | Six-hour per-request budget, bounded connection/header waits, 60-second body idle timeout, three attempts preserving partial bytes with validated HTTP ranges. A server ignoring Range restarts cleanly. | HTTPS enforced across redirects; final trusted SHA-256 and archive validation are still required. Resume is within the current preparation attempt, not across app restarts. |
| F03: overlapping update/restore/config writes | OS-backed global maintenance lock shared by Player/Developer, covering update/repair, restore/deletion, Safe Launch, source packaging, verification, launch admission, settings edits and storage cleanup. Settings use an OS-locked read/modify/write transaction. Close/restart admission refuses active work. | Conservatively serializes all pack maintenance, including overlapping roots. Safe Launch holds its guard until restoration completes. Forced process termination/power loss is not made safe by a lock. |
| F04: stale profile approvals and async responses | Remount Update/Smart Launch sessions when profile or installation changes, invalidate late responses, bind native apply/restore to reviewed and selected profile IDs, disable navigation during those workflows, show the reviewed target folder. | No old response can auto-launch a previously selected pack. Native checks remain authoritative even if UI controls are bypassed. |
| F05: interrupted configuration rotation loses settings | Recover last committed backup, then valid staged settings, before creating defaults; retain invalid JSON for diagnosis; never downgrade future schemas or replace unreadable/redirected state. Preserve native installed-version state when saving stale settings forms. | Recovery files remain available during recovery. This is settings recovery, not replay of an interrupted modpack transaction. |

The 65-second network regression initially exposed a second deadline inherited from ureq's preceding timeout phase. The final configuration leaves `recv_response` unset, bounds response headers from `send_request`, and uses `recv_body` as a per-read idle limit. The pinned ureq version and regression make that dependency behavior explicit.

## Verification

Automated network fixtures live only under Rust `#[cfg(test)]`; frontend fixtures live only in test files. They are not production mock data.

- Frontend: 30 tests passed across 13 files; TypeScript and both edition builds passed.
- Developer Rust: 108 passed, four intentionally ignored (long download, live Minecraft, live 7DTD, subprocess-only lock child). Player Rust: 83 passed, three intentionally ignored (no Developer packager test). The cross-process parent test explicitly executes the lock child twice. Close admission also remains blocked during helper startup and is only released after verified readiness.
- Both edition Clippy runs passed with `--all-targets -- -D warnings`.
- The explicitly invoked slow transfer passed in 65.03 seconds, beyond the former 60-second deadline. Interrupted/resumed transfers, ignored ranges, malformed ranges and stalled bodies pass in the standard suite.
- The actual public Minecraft package (1,026,145,390 bytes) downloaded through the new HTTPS path and installed into a disposable folder; all 2,067 tracked files verified. Damaging one disposable file then produced exactly one staged repair, with a clean complete re-verification and an untracked save sentinel preserved. The full install/repair test passed in 233.99 seconds. It invokes the native transaction engine with a no-op configuration callback; configuration persistence is covered separately. It does not test third-party launcher import or gameplay.
- Both Windows NSIS installers and win-unpacked EXEs were rebuilt as 0.1.8 after the final timeout correction. The packaging source/dist scans found no production mock data and verified edition separation.
- Both packaged editions created responsive windows with correct titles and real isolated configuration/activity files.
- The packaged Player helper replaced different bytes, retained the exact old backup, verified SHA-256 and restarted under each of the three supported executable names. Separate packaged probes verified readiness while the parent stayed alive and rejection of a corrupted staged executable before readiness or target mutation.
- This checkpoint does not install either NSIS package on a clean machine or publish a public GitHub app/modpack release.

### Built artifacts

Paths below are relative to `artifacts/windows/`; `build-manifest.json` records their absolute locations. These are generated local artifacts, not files committed into Git.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.8.exe` | 5,265,321 | `053F17C8896CB7254B3A9892752BA611885010F5FFAC1992E4C37063B63B5768` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 17,684,992 | `4CA60085D11968060420F7209FD6D8E472198FC4A3A1DE4C8DC376673A360F0A` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.8.exe` | 5,428,193 | `5CDBA5DA3731234D76A2FE114BCE9E2BAA635FCDEA2215D1E2C73DF5E20B58C9` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 18,545,664 | `AB803256E45EDB2F9724C9973A68F441EB2FF0AD42C2EC7482D90A562D67956C` |

Reproducible checks:

```powershell
npm test
cargo test --manifest-path src-tauri/Cargo.toml --all-features
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo clippy --manifest-path src-tauri/Cargo.toml --all-features --all-targets -- -D warnings
cargo clippy --manifest-path src-tauri/Cargo.toml --no-default-features --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml progressing_download_survives_old_sixty_second_limit -- --ignored
cargo test --manifest-path src-tauri/Cargo.toml live_minecraft_install_and_single_file_repair_preserve_untracked_saves -- --ignored --nocapture
npm run package:windows
npm run smoke:windows
npm run smoke:app-update
powershell -NoProfile -File scripts/Smoke-AppUpdateHelper.ps1 -TargetFileName "Mythic Loot Launcher Player.exe"
powershell -NoProfile -File scripts/Smoke-AppUpdateHelper.ps1 -TargetFileName "mythic-loot-launcher.exe"
powershell -NoProfile -File scripts/Smoke-AppUpdateHelper.ps1 -ReadinessOnly
powershell -NoProfile -File scripts/Smoke-AppUpdateHelper.ps1 -RejectCorruptStage
```

Do not recompile an edition's test executable while a long-running test from that same executable is running: Windows keeps it locked. Run the long native tests sequentially per edition.

## 7 Days to Die: what the owner needs to provide

The source is `C:\Program Files (x86)\Steam\steamapps\common\7 Days To Die\Mods`. The public destination repository is `HixxyDubz/Mythic-Loot-7DTD-Modpack`. No manually written JSON is required.

1. Confirm the exact game version shown on the main menu that friends must install.
2. Confirm which Mods snapshot is ready and authorized to be shared; the current live folder is not assumed to be an approved release snapshot.
3. Choose a modpack release version and brief change notes when publishing.

Developer Publisher can then scan the chosen folder, build the package (multipart when necessary), generate relative file paths/checksums/download URLs in the manifest, show the review, and upload after explicit confirmation. Publish the updated catalogue afterward if its public listing changes. Existing Player installations refresh the catalogue and manifests, then use the reviewed update action; they do not need the app reinstalled for new pack content.

The currently published 7DTD manifest has no tracked files or downloadable package. Until the first actual package is published and tested, 7DTD distribution remains blocked.

## Distribution and remaining work

Both 0.1.8 editions are intended for local/friend testing, not a zero-bug or full-parity certification. The installers must still be exercised on a clean Windows account/machine; real CurseForge/Modrinth import and game launch remain acceptance gates. Review findings F06-F10 and other parity gaps are listed in PARITY_STATUS.md.

Building and pushing source do not publish a new Player update feed. Public GitHub release remains 0.1.7 until the reviewed 0.1.8 Player assets/feed are deliberately published. Do not silently upload the live Mods folder or switch the public feed during a code-only fix pass.

An existing 0.1.7 **hyphenated portable** cannot repair its own old helper by merely downloading a newer candidate. Close it and rename it to `Mythic Loot Launcher Player.exe` before taking the next published update, or start testing with the fixed 0.1.8 build. This one-time legacy limitation does not affect the filenames shipped by the fixed helper.
