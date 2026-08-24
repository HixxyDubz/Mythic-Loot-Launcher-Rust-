# MLLP to Tauri migration contract

This is the behavioral contract for the rewrite. The adjacent Python project is read-only reference material; no feature is considered ported because a similar screen exists.

## Product boundary

Mythic Loot Launcher answers a friend's question: **Can I play on this private server right now?** The principal flow is server profile -> detect or configure game -> verify readiness -> safely update/repair when required -> launch -> help join.

It remains a small private launcher. It is not a marketplace, social network, credential store, or general remote server administrator.

## Required parity areas

| Area | MLLP behavior to preserve |
|---|---|
| Profiles | Multiple isolated server profiles, built-in and owner-defined games, artwork, selection and atomic persistence |
| Games | Minecraft, 7 Days to Die, Palworld, Core Keeper, Marvel Heroes, Valheim, Factorio, Stardew Valley, Hytale, World of Warcraft, RuneScape, City of Heroes, plus conservative custom adapters |
| Detection | Game/client location, install/pack destination, Steam libraries, CurseForge/Modrinth/official Minecraft layouts, Java runtimes, and manual override |
| Readiness | Honest Ready, Setup Required, Game Path Missing, Update Required, Repair Needed, Server Offline, Checking and Failed states |
| Smart Play | One orchestrated check -> update/repair -> recheck -> launch route that never launches after a failed readiness gate |
| Server status | Minecraft Server List Ping and 7DTD A2S with timeout/caching; unsupported protocols report Not Checked rather than false Offline |
| Updates | HTTPS/dedicated sources, partial downloads/retry, checksums, multipart reassembly, safe archive validation, staged overlay, disk check, exact backup, apply, obsolete removal, post-verification and complete rollback |
| Repair | Changed-file-only isolated staging and hashes; never silently replace Repair with a full archive update |
| Recovery | Backups, restore points, retention, Safe Launch crash recovery, optional extras, and all-or-nothing archive validation |
| Player tools | News, rules, changelog, folders, Discord invite, activity, storage cleanup, crash helper and redacted support bundles |
| Owner tools | Fail-closed local password gate, content editing, manifest/package preview, privacy audit, immutable GitHub Release publishing, invitations, and separate Player/Developer distributions |
| App update | Checksum-protected reviewed release feed, safe bootstrap replacement, restart, backup and rollback |
| Desktop quality | Borderless native drag/minimize/maximise/close, responsive rendering, low idle CPU, branded packaging and installed-app smoke tests |

## Non-negotiable safety rules

1. Validate and safely resolve every manifest and archive path.
2. Never use a Discord invitation or general web page as an update source.
3. Never mutate a live install before the complete candidate overlay is staged and verified.
4. Journal every created path and restore both overwritten and deleted paths on rollback.
5. Keep tokens and secrets out of React, config files, logs, reports, and public builds.
6. Owner publishing remains preview-first and uses authenticated GitHub CLI; it does not store a GitHub token.
7. Keep writable state under the Tauri application data directory (or explicit portable override), never inside bundled resources.
8. Player builds exclude owner paths, credentials, repositories, source folders and controls by construction.

## Acceptance language

- **Verified**: exercised by an automated test or controlled runtime acceptance.
- **Local only**: verified without a real external server/account/release.
- **Foundation**: types or UI exist, but the full route is not complete.
- **Not implemented**: no working route yet.
- **Blocked by configuration**: implementation exists but requires real owner-supplied values.
