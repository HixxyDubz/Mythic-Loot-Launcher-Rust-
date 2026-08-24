# Tauri rewrite parity status

Last updated: 2026-08-24

The Python launcher remains the behavior reference. This file records the new Rust/Tauri implementation only.

| Area | Status | Evidence / next gate |
|---|---|---|
| Tauri 2 desktop shell | Verified local foundation | Release build completed. Diagnostic runtime created the main webview and reported `visible=true`; the final release stayed responsive under an isolated data root. The command host does not expose its Win32 window handle, so a user-session installed-app capture remains a later packaging gate. |
| React/TypeScript interface | Verified local foundation | Responsive dashboard and Settings route visually checked at 1180x760 and the 940x640 minimum with no console errors. Vitest interaction and production build pass. |
| Rust configuration store | Verified local foundation | Tests cover defaults, round-trip persistence, corrupt-file preservation, readiness, Steam VDF parsing and launch arguments. The prior release created schema 1 with two canonical profiles under an isolated override. |
| Built-in catalogue | Foundation | All twelve MLLP game IDs and custom fallback metadata are represented. Full adapter behavior is not ported. |
| Minecraft detection | Foundation | Common CurseForge, official and launcher locations are scanned. Full 85-install parity and metadata selection are not yet proven. |
| 7DTD/Steam detection | Foundation | Default and VDF-listed Steam libraries are scanned. Runtime acceptance is pending. |
| Trusted manifest contract | Verified local | The actual Minecraft v1.0.1 inventory (2,067 required files and two obsolete paths) and 7DTD v1.0.0 manifest are compile-time bundled. Version 1 validation rejects unsafe paths, Windows aliases/ADS, invalid hashes, case collisions, obsolete overlap, bad ports and unsupported URLs. |
| Required-file verification | Verified local | Rust streams SHA-256 comparisons after safe path resolution and reports current, missing, changed and unsafe entries without modifying the install. Dashboard action is enabled after a pack directory is configured. |
| Readiness | Foundation | Game executable, pack directory, trusted manifest and manifest-sourced version gates are live. A completed file check can surface Repair Needed in the current session. Runtime/Java and Smart Play orchestration remain. |
| Native game launch | Foundation | Narrow Rust command, validated executable and game-specific direct-join args exist. Real game acceptance remains blocked by local configuration. |
| Settings/profile editing | Foundation | Selected profile fields persist through Rust. Full owner-only server creation and validation remain. |
| Smart Play | Not implemented | Requires manifest, updater/repair and server-status ports first. |
| Transactional update/repair | Not implemented | Must be ported with complete stage/backup/apply/verify/rollback contract. |
| Backups/restore/Safe Launch | Not implemented | No parity claim. |
| Server protocols | Verified local / external acceptance pending | Native Minecraft Server List Ping and 7DTD A2S info, including A2S challenge flow, 2.5-second timeouts and a 45-second cache, are covered by local protocol fixtures. Blank endpoints remain Not Checked rather than false Offline. Real private endpoints are intentionally absent. |
| News/rules/changelog/activity/storage/support | Not implemented | No parity claim. |
| Owner auth/content/publishing/invites | Not implemented | No parity claim. |
| Self-update | Not implemented | New repository feed and production replacement acceptance are required. |
| Player/Developer packaging and privacy audit | Not implemented | Must be separate artifacts, not a runtime toggle alone. |

## Verification snapshot

- `cargo test --manifest-path src-tauri/Cargo.toml`: 18 passed, including bundled manifests, adversarial paths, streaming hashes, Minecraft ping and A2S challenge fixtures.
- `npm run test`: 1 passed.
- `npm run build`: TypeScript and Vite production build passed; 1,828 modules transformed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: passed.
- `npm run tauri build -- --no-bundle`: fresh native release built successfully.
- Release executable: 11,667,968 bytes; SHA-256 `B8B308CA8354E756E3607BB980F92CE416C5CA52527BAA09531A431B35F410F1`.
- Isolated-data native smoke: responsive at the four-second sample, 27.6 MB working set, config schema 1, two profiles, selected `minecraft_main`; the temporary data folder was removed afterward.
- Browser visual check: 1180x760 and 940x640 minimum passed without horizontal overflow or console errors; content-only vertical scrolling remained available.

## External acceptance boundary

The checked-in profiles intentionally contain no private server addresses. The protocol implementations are locally verified, but real server reachability, direct join, and friend-machine acceptance cannot be claimed until the owner supplies those values and a two-machine acceptance run is completed. Remote manifest retrieval and transactional update/repair are also not part of this checkpoint.
