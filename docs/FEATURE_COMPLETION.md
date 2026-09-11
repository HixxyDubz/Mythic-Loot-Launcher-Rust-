# Rust feature-completion work

Rust is the maintained application. Python is read-only behavioral reference, never an upgrade target. This checklist tracks implementation separately from real external acceptance. No box means complete until its route and regression checks work.

## Implementation sequence

- [x] Per-release selectable game version and source folder; persistent Developer choices; native folder/file pickers (0.1.9; OS dialog interaction still needs a human smoke).
- [x] Catalogue archival reconciliation without deleting player installations; remote HTTPS artwork enabled by CSP (0.1.9; remote rendering still needs a visual check).
- [x] Separate newly saved unpublished content drafts from downloaded manifests (0.1.9).
- [x] Explicit Minecraft loader identity authoring for arbitrary profiles and both launcher bootstrap formats (0.1.9; real imports remain an external check).
- [ ] Automatic discovery of Minecraft version/loader metadata from the chosen source, without copying launcher account state.
- [ ] Optional inventory publishing, player extras management, update preservation and Safe Launch integration.
- [ ] Java/runtime and memory controls, complete server-free game-folder adapters and local game-version checks.
- [ ] User preferences: update-check behavior, close-after-launch, reduced motion, theme/font/background/window geometry, opt-in presence.
- [ ] Crash/session tracking, automatic recovery prompts, log diagnosis and support export.
- [ ] Native progress/cancellation and recoverable interrupted file transactions.
- [ ] Re-run adversarial regressions, rebuild both editions, packaged acceptance, and push tested source.

## External acceptance (not implied by code completion)

- [ ] Owner-approved 7DTD release snapshot published and downloaded in an isolated test.
- [ ] Real CurseForge and Modrinth imports and first game launch.
- [ ] Clean-machine installer execution and friend testing.
- [ ] Explicitly published Player app update feed and live old-to-new transition.

The publisher must ask for the release's game version and selected source folder, not a permanently hard-coded 7DTD version. Preparing a local preview is not permission to upload live files. Servers remain out of scope.

## 0.1.9 checkpoint

Per-release choices are saved in Developer-only `publishing-choices/<profile-id>.json`, not in the public profile or manifest. The game-version box offers saved previous choices and also accepts a new version. The source is independent of the player's deployment folder. Existing optional entries keep their classification during packaging; adding/removing optional selections in Player is not implemented by this checkpoint.

New content saves use presentation-only `content-drafts/<profile-id>.json`; packaging overlays them onto the latest verified distribution inventory. Refresh never writes this draft. This does not automatically recognize pre-0.1.9 edits that were stored inside the same manifest as downloaded content; legacy-draft recovery remains to be addressed before declaring full migration parity.

See `RELEASE_SETUP_0.1.9.md` for testing and distribution boundaries. The remaining unchecked items are actual remaining work, not a feature-complete claim.
