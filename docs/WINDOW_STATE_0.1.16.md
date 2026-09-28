# Windows window placement: 0.1.16

Both editions now expose **Settings → Launcher preferences → Remember window size and position**, enabled by default with backwards-compatible configuration loading. Each edition remembers its normal window bounds and maximized state after a graceful close. Closing while minimized never causes an invisible/minimized next start; the last non-minimized placement is retained. Disabling the preference opens a centered default-sized window next time and leaves the prior saved placement untouched.

## Behavior and safety

- Geometry is separate from profiles in `window-state.json` under the edition's data directory. The existing `MYTHIC_LOOT_DATA_DIR` override also isolates this file during testing.
- Coordinates use the native Windows placement API, preserving normal bounds while maximized and converting between screen and workspace coordinates. Logical dimensions adapt to the selected monitor's DPI. Missing monitors fall back to the primary work area; size and position are bounded to the available work area.
- Invalid, unsupported or oversized state is ignored at startup. On the next enabled save, malformed regular files are preserved as `window-state.invalid-<timestamp>.json` before replacement. Profile settings are not rewritten as part of placement recovery.
- Reads are bounded to 4 KiB, numeric fields are validated, and redirected paths are rejected. An exclusive nonblocking lock coordinates placement writers. Saving uses the existing synced atomic-write helper; a directory at the state filename is never moved or removed.
- Existing active-operation close guards remain in force. Window placement is best-effort, not a reason to prevent startup. It saves on approved normal close/exit, not every move and not on a forced process kill. This is **not crash/session recovery or interrupted modpack-operation recovery**.
- Implemented for Windows only. No new dependency package, signing keys, live game changes, production mock data or public release/feed changes.

Native API basis: Microsoft's [WINDOWPLACEMENT documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-windowplacement) and [GetWindowPlacement documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowplacement).

## Verification

- Frontend: 78 tests across 21 files passed; TypeScript type-check passed.
- Rust: Developer 177 passed, six intentionally ignored; Player 137 passed, five intentionally ignored. Both editions passed strict Clippy. Ignored external/large-package cases were not rerun.
- Eight new native regressions cover normal/maximized placement, disconnected screens, small work areas, negative coordinates/DPI changes, invalid numeric/schema data, profile isolation, malformed/oversized files, concurrent writers and directory destinations.
- Both 0.1.16 NSIS installers and win-unpacked EXEs built; production mock-data and Player edition-separation checks passed. Locked Cargo metadata and all package version records agree.
- `npm run smoke:window-state` passed against both packaged editions in isolated data directories: normal bounds survive restart, maximized state retains normal bounds, closing minimized reopens maximized, disabled persistence neither restores nor writes, off-screen coordinates recover, and malformed state is preserved without changing profile configuration. The test uses physical sizes scaled to the actual window DPI (150% on the test display); initial harness errors from forcing a below-minimum window and unavailable PowerShell 5 hashing cmdlets were corrected before the successful rerun.
- Both packaged editions passed responsive-window/native-activity startup and preference persistence checks, including disabled startup update checks. Player's packaged update helper passed different-byte replacement, exact backup preservation, activated hash verification and restart; readiness-only and corrupt-stage rejection checks also passed. These isolated tests do not constitute a live GitHub old-to-new update or clean-machine installer acceptance. Disposable smoke directories were removed; live launcher/game data was not used.

### Verified artifacts

All four sizes and hashes were independently re-read and match `artifacts/windows/build-manifest.json`. Binaries are local outputs, not committed source files. Paths below are relative to `artifacts/windows`.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.16.exe` | 5,479,517 | `4F09DA0F4503CD01AFBC1E99EC2DE2143A5D68955FFD0276C01A5B85B67399A1` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,762,752 | `E528324F84F8A0A7D4C58824C0C8508D00139B068EEC3292E79C5A199E391AB4` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.16.exe` | 5,708,230 | `BD4129DAFC4B282F410C73E1E926DFC9BB3EE78A78352D0DD8B8C6584D52BF72` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,790,336 | `428F084FFD6B7EA54490A009374DD948C2C8AD7E616D47A72496253436A0CAAD` |

Custom backgrounds, opt-in presence, broader crash/session recovery, native progress/cancellation, hard-interruption transactions and the external acceptance gates remain open. See [the completion checklist](FEATURE_COMPLETION.md). Mixed-DPI physical-monitor transitions and clean-machine installer execution still need human acceptance; calculated DPI tests are not a substitute for that hardware check.
