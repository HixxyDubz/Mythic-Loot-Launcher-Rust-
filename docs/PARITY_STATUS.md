# Tauri rewrite parity status

Last updated: 2026-08-24

The Python launcher remains the behavior reference. This file records the new Rust/Tauri implementation only.

| Area | Status | Evidence / next gate |
|---|---|---|
| Tauri 2 desktop shell | Verified local foundation | Release build completed. Diagnostic runtime created the main webview and reported `visible=true`; the final release stayed responsive under an isolated data root. The command host does not expose its Win32 window handle, so a user-session installed-app capture remains a later packaging gate. |
| React/TypeScript interface | Verified local foundation | Responsive dashboard, Settings, Publisher and Update & Repair routes visually checked at 1180x760 and the 940x640 minimum. The update workspace has no horizontal overflow at either size. Vitest interaction and production build pass. |
| Rust configuration store | Verified local | Schema 2 contains server-free modpack profiles. Schema 1 migrates atomically while preserving user paths and converts the two old default names. Tests cover defaults, migration, round-trip persistence and corrupt-file preservation. |
| Built-in catalogue | Foundation | All twelve MLLP game IDs and custom fallback metadata are represented. Full adapter behavior is not ported. |
| Minecraft detection | Foundation | Common CurseForge, official and launcher locations are scanned. Full 85-install parity and metadata selection are not yet proven. |
| 7DTD/Steam detection | Foundation | Default and VDF-listed Steam libraries are scanned. Runtime acceptance is pending. |
| Trusted manifest contract | Verified local | The actual Minecraft v1.0.1 inventory (2,067 required files and two obsolete paths) and 7DTD v1.0.0 manifest are compile-time bundled. Version 1 validation rejects unsafe paths, Windows aliases/ADS, invalid hashes, case collisions, obsolete overlap and unsupported URLs. Legacy server metadata is ignored. |
| Required-file verification | Verified local | Rust streams SHA-256 comparisons after safe path resolution and reports current, missing, changed and unsafe entries without modifying the install. Dashboard action is enabled after a pack directory is configured. |
| Readiness | Foundation | Game executable, pack directory, trusted manifest and manifest-sourced version gates are live. No network/server state participates. A completed file check can surface Repair Needed in the current session. Runtime/Java and Smart Launch orchestration remain. |
| Native game launch | Foundation | Narrow Rust command, validated executable and user-supplied launch arguments exist. The launcher generates no connection or direct-join arguments. Real game acceptance remains blocked by local configuration. |
| Settings/profile editing | Foundation | Modpack identity, local paths, versions and dedicated GitHub release URLs persist through Rust. Modpack creation is not wired yet. |
| Smart Launch | Not implemented | Transaction primitives now exist, but launch/update orchestration, runtime selection and user-facing recovery choices remain. Server behavior is explicitly out of scope. |
| Transactional update/repair | Verified local foundation / external acceptance pending | Rust supports trusted local or HTTPS packages, retries with partial files, multipart assembly, package/member integrity checks, isolated changed-file staging, disk-space gates, cached mutation previews, explicit confirmation, pre-change backup, obsolete removal, complete live post-verification and rollback. Seven updater tests cover update, changed-file-only repair, unsafe archive rejection, confirmation, staging isolation and two rollback paths. A real production GitHub package has not yet been applied. |
| Backups/restore/Safe Launch | Foundation | Every applied transaction creates a pre-change ZIP and rollback is automatic on failure. Backup browsing, retention, explicit manual restore and Smart/Safe Launch orchestration are not implemented. |
| Server handling | Explicitly out of scope | The earlier protocol experiment was removed following owner clarification. There are no server commands, fields, readiness gates, dashboard controls or generated join arguments. |
| News/rules/changelog/activity/storage/support | Not implemented | No parity claim. |
| GitHub repository creation | Foundation / external acceptance pending | Developer UI and Rust commands perform read-only `gh` preflight, validate strict `owner/name` input, default to private, preview the external mutation, and fail closed without explicit confirmation. No repository was created during local verification and no token is stored. |
| GitHub release packaging/upload | Verified local foundation / external acceptance pending | Rust walks a selected source without following links, excludes runtime/private state, scans supported text through 32 MiB, rejects personal/credential-shaped content, streams SHA-256, produces sorted fixed-metadata ZIPs and validated v1 manifests, previews inventory/diff/hash/output, caches native asset paths, and re-hashes them before an explicitly confirmed immutable `gh release create`. Tests never call GitHub; authenticated real-repository publication remains unproven. Single ZIP assets at or above 2 GiB fail closed until multipart publishing is ported. |
| Self-update | Not implemented | New repository feed and production replacement acceptance are required. |
| Player/Developer packaging and privacy audit | Not implemented | Must be separate artifacts, not a runtime toggle alone. |

## Verification snapshot

- `cargo test --manifest-path src-tauri/Cargo.toml`: 30 passed, including schema/config recovery, bundled manifests, adversarial paths, streaming hashes, deterministic publishing, privacy scanning, fail-closed GitHub actions, changed-file repair, staging isolation, confirmation, and transaction rollback.
- `npm run test`: 3 passed.
- `npm run build`: TypeScript and Vite production build passed; 1,830 modules transformed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`: passed.
- `npm run tauri build -- --no-bundle`: fresh server-free native release built successfully.
- Release executable: 16,312,320 bytes; SHA-256 `A695E1AF5EFDE20A5D44BE9F650463F3F4CBAE7246A539920E8A6D1ADA489DE0`.
- Isolated-data native smoke: responsive at the four-second sample, 29.8 MB working set, schema 2, two profiles, and no `serverName`, `serverIp` or `serverPort` keys; temporary data was removed afterward.
- Browser visual check: the Update & Repair workspace passed at 1180x760 and 940x640 without horizontal overflow; its safety sequence and preparation actions remained readable at both sizes.

## External acceptance boundary

Server acceptance is not a launcher gate because servers are managed elsewhere. A full real-package GitHub update/repair, authenticated GitHub repository creation/release publication, multipart publication, separate Player/Developer artifacts, manual backup restore and friend-machine modpack acceptance are not part of this checkpoint.
