# Rust feature-completion work

Rust is the maintained application. Python is read-only behavioral reference, never an upgrade target. This checklist tracks implementation separately from real external acceptance. No box means complete until its route and regression checks work.

## Implementation sequence

- [x] Per-release selectable game version and source folder; persistent Developer choices; native folder/file pickers (0.1.9; OS dialog interaction still needs a human smoke).
- [x] Catalogue archival reconciliation without deleting player installations; remote HTTPS artwork enabled by CSP (0.1.9; remote rendering still needs a visual check).
- [x] Separate newly saved unpublished content drafts from downloaded manifests (0.1.9).
- [x] Explicit Minecraft loader identity authoring for arbitrary profiles and both launcher bootstrap formats (0.1.9; real imports remain an external check).
- [x] Read-only Minecraft version/loader discovery on request from the chosen source, without copying launcher account state (0.1.12; CurseForge instance/export and extracted Modrinth export JSON; database-only Modrinth instances remain manual/unknown).
- [x] Optional inventory publishing, Player extras management, update preservation and Safe Launch protection (0.1.10; controlled file-transaction tests, not gameplay acceptance).
- [x] Recover surviving pre-0.1.9 content from the cached manifest; preserve it before Developer refresh, and provide reviewed draft recovery/discard controls (0.1.13). Already overwritten text without a surviving copy cannot be reconstructed; cached text is not automatically classified as unpublished.
- [x] Read-only Java discovery and advanced direct-Java memory argument controls (0.1.11); CurseForge/Modrinth own their runtime settings and have linked setup guidance, not hidden configuration writes.
- [x] Read-only cross-game path/client checks, separate Steam installed-build evidence, Factorio base-version comparison, configured-target preservation and missing legacy client candidates (0.1.14).
- [x] Hytale default-root patchline-specific mod targets and Factorio declared system/portable/custom targets, including supported absolute command-line overrides (0.1.15). Detection is read-only and reviewed; unsupported/missing configuration remains manual.
- [ ] Remaining server-free adapter parity: additional custom Hytale root discovery, broader game-version readers where reliable metadata is available, and real adapter gameplay acceptance. Unknown versions must remain unknown; third-party runtime automation needs reviewed integration.
- [x] Startup update-check behavior, explicit catalogue refresh, close-after-normal-launch, reduced motion, colour theme, font and plain/decorative backgrounds (0.1.11).
- [x] Optional edition-local Windows geometry persistence, maximized-state restore and off-screen/malformed-state fallback (0.1.16).
- [ ] Custom background selection and opt-in presence.
- [ ] Crash/session tracking, automatic recovery prompts, log diagnosis and support export.
- [ ] Native progress/cancellation and recoverable interrupted file transactions.
- [ ] Re-run adversarial regressions, rebuild both editions, packaged acceptance, and push tested source.

## External acceptance (not implied by code completion)

- [ ] Owner-approved 7DTD release snapshot published and downloaded in an isolated test.
- [ ] Real CurseForge and Modrinth imports and first game launch.
- [ ] Clean-machine installer execution and friend testing.
- [ ] Explicitly published Player app update feed and live old-to-new transition.

## Security follow-up

- [x] Install cargo-audit and patch rustls 0.23.43 to 0.23.45 for RUSTSEC-2026-0285 (0.1.15).
- [ ] Review upstream advisory warnings as dependencies evolve; the audit is not a zero-risk certificate.
- [ ] Cryptographically authenticated app updates and modpack/catalogue metadata. Current transport/hash checks do not protect against compromised publisher credentials. Local key-based update signing is separate from paid Windows publisher certificates; no keys were generated in 0.1.15.
- Windows Authenticode signing is deliberately out of scope per the owner's preference for private friend testing.

The publisher must ask for the release's game version and selected source folder, not a permanently hard-coded 7DTD version. Preparing a local preview is not permission to upload live files. Servers remain out of scope.

## 0.1.9 checkpoint

Per-release choices are saved in Developer-only `publishing-choices/<profile-id>.json`, not in the public profile or manifest. The game-version box offers saved previous choices and also accepts a new version. The source is independent of the player's deployment folder. Existing optional entries keep their classification during packaging; adding/removing optional selections in Player is not implemented by this checkpoint.

New content saves use presentation-only `content-drafts/<profile-id>.json`; packaging overlays them onto the latest verified distribution inventory. Refresh never writes this draft. This does not automatically recognize pre-0.1.9 edits that were stored inside the same manifest as downloaded content; legacy-draft recovery remains to be addressed before declaring full migration parity.

See `RELEASE_SETUP_0.1.9.md` for testing and distribution boundaries. The remaining unchecked items are actual remaining work, not a feature-complete claim.

## 0.1.10 checkpoint

`OPTIONAL_EXTRAS_0.1.10.md` records the next completed implementation slice. Developer can mark relative files/folders optional. Player can review enabling/disabling extras in Update & Repair. Selections are committed with the installed version in local configuration, scoped to the installation folder. Updates now extract only manifest-listed files and enabled extras, never every ZIP member indiscriminately. Safe Launch session recovery takes precedence over maintenance.

This does not complete the unchecked runtime/preferences, crash/recovery, progress/cancellation or real external acceptance work above.

## 0.1.11 checkpoint

`PREFERENCES_RUNTIME_0.1.11.md` records the working preferences and runtime slice. Preferences persist locally with backwards-compatible defaults and apply to real behavior. Normal launches can request a guarded close after success; Safe Launch stays open. Java discovery reads local release metadata without executing found binaries. The advanced memory editor changes only JVM heap arguments before the Java entry point, requires a direct Java executable, and leaves the result in the settings draft for review/save. Minecraft launchers continue managing their own Java and memory settings.

The remaining unchecked implementation and external-acceptance items still prevent a feature-complete claim. No public release/feed is created by building or pushing this checkpoint.

## 0.1.12 checkpoint

`MINECRAFT_METADATA_0.1.12.md` records bounded, read-only metadata discovery and advisory requirement checks. Publisher applies only the reviewed version/loader to the unsaved draft and invalidates previous release approval. Settings compares recognised declarations with the native verified published manifest, never an unpublished authoring draft. Missing, malformed or conflicting files stay unknown. Export snapshots alone cannot prove installed compatibility. Modrinth's private database and archive files are not opened. Metadata does not gate sync/launch or prove gameplay compatibility.

The existing real CurseForge source was read successfully and its metadata SHA-256 stayed unchanged. Launcher/export root metadata is excluded from Minecraft packages without excluding nested game files or other games' manifest.json. Legacy-draft recovery/discard controls are the next implementation item; the remaining checklist and external acceptance are still open.

## 0.1.13 checkpoint

`CONTENT_RECOVERY_0.1.13.md` records Developer-only local content recovery. Existing cached presentation fields are preserved before a changed downloaded/published manifest replaces them. Saved drafts retain previous versions and can be explicitly discarded with a recovery copy. Recovery requires reviewed native identifiers, an unchanged saved-draft revision and confirmation; it copies only presentation fields over the current authoring base. Cached text may have been published already, so no automatic recovery/classification occurs. Malformed drafts can be preserved/discarded but not blindly applied. No public upload is part of recovery.

Remaining work includes broader server-free game-folder/local-version checks, geometry/background/presence, crash/session recovery and native progress/cancellation/interruption recovery, plus the external acceptance gates above.

## 0.1.14 checkpoint

`INSTALLATION_CHECKS_0.1.14.md` records advisory local installation checks in both editions. Configured game roots and exact modpack targets remain separate; selecting a candidate cannot append `Mods` twice or retain an unrelated old executable when the new one is missing. Known nested Factorio/Palworld/Marvel Heroes executable layouts resolve to the game root. Steam build IDs remain distinct from user-facing game versions. Factorio's base metadata can be compared with an exact published version. Missing, unsupported or ambiguous evidence never implies version compatibility. Real 7DTD read-only acceptance found the client/build without modifying its Steam metadata. This is a partial adapter checkpoint, not completion of every game's version reader or target routing.
