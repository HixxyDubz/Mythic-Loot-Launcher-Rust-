# Minecraft metadata checkpoint: 0.1.12

Implemented and verified on 2026-09-16. Rust only; Python/MLLP is unchanged. This checkpoint is local test artifacts plus source, not a public app-feed or modpack publication.

## Using the feature

In Developer **Publisher**, select the modpack source folder and click **Inspect Minecraft metadata**. Review the detected Minecraft version, loader and evidence. **Use detected version and loader** copies only those two strings into the unsaved release draft and invalidates an existing release preview. Saving choices, preparing a package and confirming upload remain separate actions. Inspection never uploads anything or changes the selected source.

In either edition's **Settings**, the same check reads the selected modpack base folder and compares its declarations against the native verified published manifest. It cannot rewrite the required version or loader. A match means declared metadata matches, not that game binaries, mods or gameplay have been validated. A mismatch is advisory; it does not add a new sync/launch enforcement rule. Missing requirements or unsupported/conflicting metadata are not reported as a match.

## Supported formats and boundaries

- CurseForge `minecraftinstance.json`: Minecraft `gameVersion` and `baseModLoader.name`, including a consistency check on the loader's declared Minecraft version when present.
- CurseForge exported `manifest.json`, format 1: Minecraft version and unambiguous primary loader.
- Extracted Modrinth `modrinth.index.json`, format 1: Minecraft and one supported loader dependency.
- NeoForge, Forge, Fabric and Quilt identities are normalised to the forms accepted by the existing launcher bootstrap. Unknown/future loaders remain manual rather than guessed. Vanilla/no-loader metadata is not supported by this modded-profile detector.

Only three fixed root files are inspected, with an 8 MiB per-file limit and link/reparse-path rejection. No executable is run. Raw JSON, unknown fields, account data, download URLs and arguments are not returned to the UI or copied into authoring state. Parse errors do not echo raw contents. Changing the selected source/profile discards prior and pending results. Supported files must agree; a valid instance file cannot hide a broken or conflicting export beside it.

An export may be old: matching export-only metadata is useful for authoring but never proof of the installed version. `.mrpack`/ZIP archives and Modrinth's private database are not read. Database-only Modrinth installations therefore require manual verification; existing Modrinth sync/bootstrap support is unchanged. Format scope is based on the [official Modrinth export specification](https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack), [Modrinth architecture documentation](https://docs.modrinth.com/contributing/theseus/) and the observed CurseForge instance metadata.

Minecraft publication now excludes root `manifest.json` and `modrinth.index.json` alongside the already excluded `minecraftinstance.json`, preventing raw export/launcher metadata from entering the package. Nested game files with those names remain eligible; other games' root manifest files are unaffected. This does not import downloaded mods from an export index: package the real installed mods/config folder, not merely an export skeleton.

## Verification

- Frontend: 60 tests across 19 files passed, including explicit draft-only application, Player read-only controls, ambiguity handling, failure after earlier success, stale folder/profile responses and release-preview invalidation.
- Native tests cover safe projection, size bounds, malformed/future/ambiguous declarations, BOM handling, normalisation, conflicts and game-scoped package exclusions.
- Developer Rust: 145 passed, five intentionally ignored in the standard suite; Player Rust: 112 passed, four intentionally ignored. The explicit read-only source test below was run separately. Previously added live-package/long-transfer cases were not rerun.
- Clippy passed both editions with `--all-targets -- -D warnings`; TypeScript type-checking passed.
- Explicit read-only acceptance against the owner's `Minecraft Very Vanilla` CurseForge folder detected Minecraft **1.21.1** and **NeoForge 21.1.248**. The metadata hash before and after was identical: `EB0767E1255EE3491B583B157AA48DE9E285D39654875BA445017402E33F7FF2`. This is metadata acceptance, not a Minecraft launch test.

- Both 0.1.12 NSIS installers and win-unpacked EXEs built successfully. Source/dist production-mock scans and Player edition separation checks passed.
- Both packaged editions passed isolated responsive-window/native-activity startup and preferences-restart checks (`npm run smoke:windows -- -VerifyPreferences`). Saved preferences survived native bootstrap and disabled startup checks stayed disabled. This is not a visual review of every new control or clean-machine installer acceptance. The smoke scripts removed only their own uniquely named disposable test folders.
- Packaged Player updater-helper checks passed: replacement/backup/hash/restart, readiness while the parent remained alive, and corrupt-stage rejection. These were disposable local tests, not a live GitHub release transition.
- All four artifact sizes and SHA-256 values were independently checked against `artifacts/windows/build-manifest.json`.

### Built artifacts

Paths are relative to `artifacts/windows`. Binaries are local outputs, not committed source files.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.12.exe` | 5,424,591 | `3B3F159BF900BF1640AB173AE132138CFD6C7C389054D311E9A9100F22B179D4` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,525,696 | `71C5AF42FEFFFEB6877DF3E3C2DB0B8C8895738C5249A993D5E01CA3502E30F9` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.12.exe` | 5,642,514 | `2906607C4E37BF9225954CBA239346B3DA7F97D79833888320122EC329EFA220` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,559,424 | `5CAB3C3764A2EBE4EBB54E47B277C8CC5817DF9969BEBFABF61633469D2317ED` |

## Remaining work

The next implementation item is legacy content-draft recovery/discard controls. The unchecked implementation and external-acceptance items in `FEATURE_COMPLETION.md` remain open. This checkpoint does not claim feature completeness, zero bugs, real launcher-import/gameplay acceptance, clean-machine installation or public update delivery. Pushing source does not update installed Players; publishing the reviewed app feed remains a separate explicit step.
