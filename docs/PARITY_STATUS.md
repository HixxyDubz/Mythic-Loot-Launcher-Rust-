# Tauri rewrite parity status

Last updated: 2026-08-24

The Python launcher remains the behavior reference. This file records the new Rust/Tauri implementation only.

| Area | Status | Evidence / next gate |
|---|---|---|
| Tauri 2 desktop shell | Verified local foundation | Release build completed. Diagnostic runtime created the main webview and reported `visible=true`; the final release stayed responsive under an isolated data root. The command host does not expose its Win32 window handle, so a user-session installed-app capture remains a later packaging gate. |
| React/TypeScript interface | Verified local foundation | Responsive dashboard and Settings route visually checked at 1180x760 and the 940x640 minimum with no console errors. Vitest interaction and production build pass. |
| Rust configuration store | Verified local foundation | Seven Rust tests cover defaults, round-trip persistence, corrupt-file preservation, readiness, Steam VDF parsing and launch arguments. Final release created schema 1 with two canonical profiles under an isolated override. |
| Built-in catalogue | Foundation | All twelve MLLP game IDs and custom fallback metadata are represented. Full adapter behavior is not ported. |
| Minecraft detection | Foundation | Common CurseForge, official and launcher locations are scanned. Full 85-install parity and metadata selection are not yet proven. |
| 7DTD/Steam detection | Foundation | Default and VDF-listed Steam libraries are scanned. Runtime acceptance is pending. |
| Readiness | Foundation | Game executable and pack directory gates are live. Manifest/version/hash/runtime/server gates remain. |
| Native game launch | Foundation | Narrow Rust command, validated executable and game-specific direct-join args exist. Real game acceptance remains blocked by local configuration. |
| Settings/profile editing | Foundation | Selected profile fields persist through Rust. Full owner-only server creation and validation remain. |
| Smart Play | Not implemented | Requires manifest, updater/repair and server-status ports first. |
| Transactional update/repair | Not implemented | Must be ported with complete stage/backup/apply/verify/rollback contract. |
| Backups/restore/Safe Launch | Not implemented | No parity claim. |
| Server protocols | Not implemented | No parity claim. |
| News/rules/changelog/activity/storage/support | Not implemented | No parity claim. |
| Owner auth/content/publishing/invites | Not implemented | No parity claim. |
| Self-update | Not implemented | New repository feed and production replacement acceptance are required. |
| Player/Developer packaging and privacy audit | Not implemented | Must be separate artifacts, not a runtime toggle alone. |

## Verification snapshot

- `cargo test --manifest-path src-tauri/Cargo.toml`: 7 passed.
- `npm run test`: 1 passed.
- `npm run build`: TypeScript and Vite production build passed; 1,828 modules transformed.
- `npm run tauri build -- --no-bundle`: release executable built successfully.
- Release executable: 10,626,560 bytes; SHA-256 `39E97CD08BA30B041DCE47CA7F68B381480780C1373A0392CCCB04E3F50A41CD`.
- Final release smoke: process responsive, 26.8 MB working set at sample, config schema 1, two profiles, selected `minecraft_main`.

## External acceptance boundary

The checked-in profiles intentionally contain no private server addresses. Real server status, direct join, and friend-machine acceptance cannot be claimed until the owner supplies those values and a two-machine acceptance run is completed.
