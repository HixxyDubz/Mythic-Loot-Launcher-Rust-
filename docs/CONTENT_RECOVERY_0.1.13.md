# Content recovery checkpoint: 0.1.13

Implemented on 2026-09-16; final package verification on 2026-09-21. Rust only. Python/MLLP, live game folders and public release feeds are not modified by this checkpoint.

## What works

Developer **Publisher → Manifest content editor → Review saved content recovery** now offers:

- A projection of the existing cached manifest's News, banner URL, Rules and Changelog, including surviving pre-0.1.9 edits that were stored alongside distribution data.
- Preserved cached presentation snapshots and previous/discarded local drafts. Choose an entry and inspect its content before explicitly confirming recovery.
- **Recover reviewed content locally** copies only these presentation fields into the local draft, revalidates them against the current authoring base, replaces unsaved editor text after confirmation and invalidates release previews. Package URLs, hashes, file inventories and versions are never restored from history.
- **Discard saved draft with recovery copy** first preserves the exact saved bytes, then removes only that profile's active local draft. The editor returns to its underlying cached content. It does not delete or refresh the cached manifest, game files or GitHub content. Use another explicit review to restore the retained draft later.

Old versions did not mark unpublished edits separately from downloaded text. Cached entries therefore say they may already have been published; they are never automatically restored or treated as proof of unpublished work. Text already overwritten without a surviving copy cannot be reconstructed.

## Preservation and safety

Developer manifest refresh and local activation of a newly published manifest preserve the previous cached presentation fields before replacement. No new snapshot is made for an unchanged manifest. Ordinary content saves also preserve a replaced draft. Recovery snapshots live under `content-recovery/<profile-id>/` within that edition's local data directory, alongside—not inside—the game or modpack. Hash-addressed immutable entries deduplicate identical content; existing history is never automatically deleted. There is no production fallback data.

A per-data-directory OS file lock coordinates content saves, refresh commits and recovery across Developer processes. Native recovery/discard requires explicit confirmation and a saved-draft revision matching the reviewed state. Recovery identifiers are validated and resolved within the selected profile's history, then rehashed before use. Cached recovery candidates are also rechecked if a refresh occurred after review. No arbitrary frontend source/destination path is accepted. Linked/reparse paths are rejected; draft/history reads are limited to 8 MiB per file and cached-manifest reads to 64 MiB. Listings inspect at most 1,000 directory entries, display at most 50 archived candidates within an 8 MiB accepted-content budget, and disclose truncation. The current cached candidate is separate from that history budget.

If a draft is malformed, its exact bytes can still be retained before discard, but the UI will not expose raw malformed content or automatically apply it. Oversized/unreadable/linked drafts remain unchanged and require manual investigation. Damaged cached manifests or recovery destinations block replacement instead of destroying the only surviving copy. A valid archived draft cannot repair invalid distribution data by itself; the authoring base must still pass validation.

The editor prevents simultaneous form edits during its operations and ignores late responses after profile changes/unmount. A successful save invalidates the prior recovery review. Recovery/discard failures require a fresh review. Player has no recovery UI, module or command registrations; packaging checks also reject these Developer IPC names in Player's JavaScript.

## Verification

- Frontend: 69 tests across 20 files passed. Coverage includes confirmation/selection, recover/discard arguments, failed/stale review reset, damaged/empty history, late-response protection and editor/release-preview invalidation.
- Native controlled tests cover preservation before refresh, unchanged refresh deduplication, projection without distribution fields, current-inventory preservation, retained replaced drafts, recoverable valid/malformed discard, stale revision/confirmation refusal, traversal/tamper rejection, damaged cache/backup refusal, size limits, profile identity and OS lock exclusion.
- Developer Rust: 152 passed, five intentionally ignored. Player Rust: 112 passed, four intentionally ignored. The explicitly gated live-source/long-transfer tests were not rerun for this checkpoint.
- Strict Clippy passed both editions (`--all-targets -- -D warnings`); TypeScript checking and locked Cargo metadata validation passed.

- Both 0.1.13 NSIS installers and win-unpacked EXEs built successfully. Production-mock scans and Player edition-separation checks passed during packaging. All four output sizes and SHA-256 values were independently verified against `artifacts/windows/build-manifest.json` on 2026-09-21.
- Both packaged editions passed isolated responsive-window/native-activity and preferences-restart checks (`npm run smoke:windows -- -VerifyPreferences`). Saved preferences survived native bootstrap and disabled startup checks stayed disabled. This is not a manual visual walkthrough of the recovery controls.
- The packaged Player updater helper passed replacement/backup/hash/restart, readiness while the parent remained alive, and corrupt-stage rejection checks. These were disposable local tests, not a live GitHub release transition. Smoke cleanup removed only the scripts' uniquely named test folders.

### Built artifacts

Paths are relative to `artifacts/windows`. Binaries are local outputs, not committed source files.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.13.exe` | 5,426,876 | `199C08F0665F57DB9E07AD619EC133B35CA62A9549E51A18BF1ED322B64CED58` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,525,696 | `4D4245D280A326270623536919C08E9B99A9A5E3EFC29D1330EA11101B56DD25` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.13.exe` | 5,671,999 | `2E399548A88C0B8C5D062F1560F3F6C85A421EC6531A931E36A87ACE9008AC07` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,583,488 | `0CA0DE19F8F474133E5B4EDD914FEFBB4A2E908597200CADC7DB794678BD024E` |

## Distribution boundary

These are local testing builds and a source update, not a public app or modpack release. No live catalogue/feed publication, real launcher import/gameplay, clean-machine installation or manual visual walkthrough is implied. See `FEATURE_COMPLETION.md` for remaining implementation and external-acceptance work. The app is not yet feature-complete.
