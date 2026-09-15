# Preferences and Java checkpoint: 0.1.11

Implementation: 2026-09-14; final verification: 2026-09-15. Rust only. Python/MLLP and live game/modpack folders were not modified. This checkpoint builds local test artifacts and pushes source; it does not publish a public app feed or modpack release.

## Working preferences

Settings now has a separate **Launcher preferences** section in both editions. Preferences apply across profiles but stay in that edition's local configuration. Native read/modify/write persistence preserves profiles, installed versions and optional-file choices. Old preference objects gain default appearance values without resetting their existing choices.

- Startup update checks default on. Turning them off prevents both automatic catalogue/manifest refresh and app-feed checks the next time the app opens. This does not cancel an already-started check or disable explicit user actions. **Refresh catalogue now** and **App update** remain available. Downloads/installation still need the existing review and confirmation.
- Close-after-launch requests a normal guarded window close only after a successful normal launch. Opening CurseForge/Modrinth also counts as a normal launch. Safe Launch uses a separate workflow and stays open for restoration; active native work can block closing. Interrupted Safe Launch records must be recovered before normal launch as well as maintenance.
- Reduced motion disables animation/transitions. The Windows/browser reduced-motion preference also takes precedence.
- Amethyst/Slate colour themes, system/Verdana interface fonts and decorative/plain backgrounds apply to the actual shell after saving.
- Late startup responses cannot overwrite a newer local state or an open settings draft. Explicit catalogue refresh also preserves edited local-path and preference drafts; clean forms adopt the refreshed state. A startup refresh that arrives after interaction updates native cached data without replacing the current screen; explicit refresh/reopening shows that cache.

## Minecraft runtime tools

**Find installed Java runtimes** reads known local runtime locations and PATH/JAVA_HOME without launching any discovered executable. Results are deduplicated and show only path, source and version/vendor/architecture declared in a bounded JDK `release` file. This is not a binary identity check or a claim that a runtime matches the modpack. Search depth/entry/time budgets limit discovery; it is not an exhaustive disk scan. Individual filesystem calls can still depend on the responsiveness of the underlying drive. Manual executable selection remains available.

For **advanced direct Java launches** only, choose java.exe/javaw.exe and supply an existing full main-class or JAR command. The memory editor offers automatic, 4/6/8 GiB and custom 512–65536 MiB. It replaces explicit heap flags before the Java entry point, preserves application arguments, quotes/empty strings and Windows paths, and refuses hidden `@argument` files rather than guessing their contents. The result stays in the modpack settings draft until **Save settings**. Automatic removes explicit heap flags; JVM ergonomics, other supplied options and environment variables can still affect memory. Direct Java now runs from Game directory (or Modpack base folder if blank), not Java's bin folder. No account, classpath, loader or Minecraft authentication command is manufactured.

**CurseForge and Modrinth still own their runtime and memory settings.** The UI links their official setup guidance and does not feed JVM flags to their launcher EXEs or rewrite their private metadata. Switching from one of those launchers to direct Java explicitly clears only the unsaved launch-target/arguments draft; it does not silently change a saved installation.

Guidance checked against [CurseForge's Java/memory settings](https://support.curseforge.com/support/solutions/articles/9000218572-getting-started), [Modrinth's Java setup](https://support.modrinth.com/en/articles/8797659-java-installations), and [Oracle's Java command/heap options](https://docs.oracle.com/en/java/javase/24/docs/specs/man/java.html). The CurseForge and Modrinth guides are also linked directly in the UI.

## Verification

- Frontend: 53 tests across 18 files passed. Tests cover real shell preference attributes, disabled startup checks/manual refresh, late-response and explicit-refresh draft protection, save failures, successful/failed close-after-launch behavior, launcher-specific memory boundaries, explicit direct-Java draft switching and stale Java editor responses.
- Developer Rust: 134 passed, four intentionally ignored. Player Rust: 102 passed, three intentionally ignored. Previously added live-package/long-transfer cases were not rerun for this checkpoint.
- Clippy passed both editions with `--all-targets -- -D warnings`; TypeScript type-checking passed.
- Controlled native tests cover old preference defaults, invalid enum rejection, preservation of profile/optional settings, non-executing discovery, Windows quoting/empty arguments, heap replacement only before the entry point, invalid memory/argument-file refusal and direct Java working-directory selection.

- Both final 0.1.11 NSIS installers and win-unpacked EXEs built successfully. Source/dist production-mock scans and the Player edition-separation checks passed.
- `npm run smoke:windows -- -VerifyPreferences` passed for both editions. Both created responsive correctly titled windows and isolated native configuration/activity. The restart phase proved native configuration loading via schema migration, preserved all six preferences and recorded no automatic catalogue/app-update checks with startup checks disabled. This is not visual acceptance of every control.
- The final packaged Player updater helper passed replacement/backup/hash/restart, readiness while its parent stayed alive, and corrupt-stage rejection checks. These were disposable local tests, not a new live GitHub update.
- All four final artifact sizes and SHA-256 values were independently checked against `artifacts/windows/build-manifest.json`. The smoke scripts cleaned only their uniquely named temporary test directories.

### Built artifacts

Paths below are relative to `artifacts/windows`. Binaries are local outputs, not committed source files.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `player/Mythic Loot Launcher Player Setup 0.1.11.exe` | 5,416,913 | `E38C800D76683C4BB84D269C64232880D3C78A113448F71F2571243908D2B2EE` |
| `player/win-unpacked/Mythic Loot Launcher Player.exe` | 18,455,552 | `9AC790C4FCC77E132921152464339F2379D2A041C335093907B6BDB091DB1229` |
| `developer/Mythic Loot Launcher Developer Setup 0.1.11.exe` | 5,606,574 | `1EC413D17D278D70BD3A26EF7263BEF01A489E77BB8CDE3F2C0F6D12C4AA8C15` |
| `developer/win-unpacked/Mythic Loot Launcher Developer.exe` | 19,398,144 | `5833014965F694CD65A406CD028AE652342C2BE92E05394C4EE52768277FC162` |

## Not yet complete

Remaining work includes local game/loader metadata discovery and version checks, legacy-draft recovery controls, window geometry/custom backgrounds/presence, further crash/recovery work and native progress/cancellation with interrupted transaction recovery. See `FEATURE_COMPLETION.md` for the implementation and external-acceptance checklist.

This checkpoint does not verify real Minecraft gameplay, actual CurseForge/Modrinth runtime settings changes, clean-machine installer execution or the new visual settings through manual packaged interaction. It does not claim feature completeness or zero bugs. The public app feed is unchanged by this work; a source push alone does not ship 0.1.11 to installed Players.
