# Optional mods checkpoint: 0.1.10

Date: 2026-09-14. Rust only. Python/MLLP and live modpack/game folders were not changed. Public app or modpack releases were not published by this work.

## Developer

In Publisher, enter optional **relative files or folders, one per line**. Folder choices expand to their publishable files. Unknown/excluded paths fail preparation; absolute paths and traversal are refused. Leaving the field empty makes all files required. At least one required file must remain.

The preview shows the optional-file count and produces distinct required/optional manifest inventories, with size and SHA-256 for every file. The ZIP contains both inventories. Existing 0.1.9 publishing choices migrate their optional selection from the current manifest when opened. Choices remain local and require the usual reviewed GitHub publication; they are not automatically uploaded.

## Player and Developer local installations

- Open Update & Repair to inspect optional files, their enabled selection, presence and hash status.
- Toggle the files, then choose **Review optional mod changes**. Preparation stages selected missing/changed files and identifies deselected files for removal. Required files are also repaired if necessary to avoid creating a mixed-version installation.
- Confirm the reviewed transaction to apply. Existing affected files are backed up; a failed file operation or configuration commit triggers rollback. Disabling an extra never means deleting its backup.
- Enabled choices and installed-version state commit together in local configuration, outside the game folder. They are scoped to the canonical installation path. Changing to another installation does not silently copy old choices.
- Fresh installations leave extras off. Existing installations without saved choices adopt optional files already present. Later updates/repairs retain those choices and repair selected files even if a file has gone missing.
- Updates extract only the required manifest inventory plus selected extras. Unlisted ZIP members and unselected extras are not installed; unrelated save files are left alone.
- Legacy entries with individual HTTPS download URLs can be staged through the same downloader and file-hash verification; otherwise preparation may download the full pack ZIP even for one extra. The live individual-download integration is not part of the controlled fixture acceptance below.
- A completed restore drops the newer selection record so subsequent inspection reflects the restored files. This is a file-backed reconstruction, not historical recovery of an enabled-but-already-missing extra.
- Active or interrupted Safe Launch sessions block file checks/update/restore until finished or recovered. Safe Launch continues to move only installed optional files temporarily and restore their recorded bytes, without changing saved choices.

## Safety and verification

Tests exercise new-install opt-in, existing-file adoption, path-scoped persisted choices, missing selected-file repair, ZIP allowlisting, preserving unrelated saves, directory substitution after review, disabling modified extras with a backup, rollback on failed configuration commit, and stale configuration rejection. Frontend tests cover explicit reviewed choices, changed-selection invalidation and ignoring late responses from another profile.

- Frontend: 38 tests across 15 files passed; TypeScript and both edition production builds passed.
- Developer Rust: 124 passed, four intentionally ignored. Player Rust: 92 passed, three intentionally ignored. The long-transfer/live-package cases were not re-run in this checkpoint; the cross-process lock parent test runs its otherwise-ignored child.
- Both editions passed Clippy with `--all-targets -- -D warnings`.
- Both NSIS installers and win-unpacked EXEs built as 0.1.10. Source/dist no-production-mock scans and Player edition separation passed.
- Both final portable apps created responsive correctly titled windows plus isolated native configuration/activity files. The guarded smoke script removed only its temporary test data.
- The packaged Player update helper replaced different reviewed bytes under the public hyphenated name, retained the exact old backup, verified SHA-256 and restarted the probe. Readiness while the parent remained alive and corrupt-stage rejection also passed.
- All four final artifacts were independently re-hashed and matched the build manifest. Installer execution on a clean machine, visual optional-list interaction, live per-file HTTPS downloads and gameplay remain external acceptance checks.

Test data exists only in test code and disposable directories, never as a production fallback.

### Built artifacts

Paths are relative to `artifacts/windows`; binaries are local build outputs, not committed source files.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.10.exe` | 5,384,572 | `E2B018E909ECB0B6C868814B3203A588FACE1283DA703151906CC291B80F15B3` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,329,600 | `8D3AD39145A34D1331C2FC10D66FCD35D320C8575EBD3A13659C0F97559F394D` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.10.exe` | 5,582,320 | `22599A93C231AAEAF8675AD3FB1EC0EEA8FFE5F662958CDCD5F848807A4B766A` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,328,000 | `C5A286ADDC3609529DCCCEFB16789EB3BFD0F3AA4AA700E4C96DC1468D9D729C` |

## Remaining work

This checkpoint does not add hard-crash transaction replay, progress/cancellation, the remaining runtime/preferences controls, legacy-content-draft recovery or real clean-machine/gameplay acceptance. See `FEATURE_COMPLETION.md`. No bug-free or feature-complete claim is made. A source push and local build do not update the public Player feed by themselves.

The public latest-release tag was rechecked on 2026-09-14 and remains v0.1.7. No new feed or modpack release was published during this checkpoint.
