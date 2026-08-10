# Changelog

All notable changes to The Staff Room are recorded here. The project follows Semantic Versioning after the public `1.0.0` release.

## [1.0.0] - 2026-08-10

### Added

- Completed packaged Windows acceptance for Ask, Quick Edit, human-gated Ship, cancellation, recovery, promotion interruption, and all supported provider routes.
- Added Windows Job Object ownership for provider process trees, including detached descendants.
- Added a fail-closed Windows release builder that remaps builder-local Rust paths and scans the compiled executable before distribution.

### Changed

- Made explicit Ship participant selection authoritative and rotated independent reviewers across Codex, Claude Code, Cursor Agent, and Antigravity.
- Updated current Antigravity model discovery and headless review handling, and made Cursor's managed-worktree trust behavior explicit.
- Prevented interrupted promotion or abandonment states from being offered as resumable, with matching renderer and Rust enforcement.
- Promoted npm, Cargo, Tauri, and installer metadata from `1.0.0-rc.1` to `1.0.0`.

### Release status

The human-gated Windows v1 contract is accepted. Autonomous Ship remains locked and fail-closed under [its separate acceptance contract](docs/AUTONOMOUS_ACCEPTANCE_CONTRACT.md).

## [1.0.0-rc.1] - 2026-08-08

### Added

- Public architecture, build, roadmap, security, contribution, and release-checklist documentation.
- MIT licensing metadata.
- A restrictive production Content Security Policy.
- Windows continuous integration for the reproducible local verification gate.
- A representative four-agent README walkthrough with a linked 1080p version.
- A startup migration that copies the pre-release SQLite database and local voice assets into the renamed application-data directory. The old data is retained, and the copied database must pass SQLite `quick_check` before use.

### Changed

- Renamed the product from Agent Room to The Staff Room, the npm and Rust packages from `agent-room` to `staff-room`, and the Tauri library to `staff_room_lib`.
- Changed the Windows bundle identifier from `com.agentroom.desktop` to `com.staffroom.desktop` and the database filename from `agent-room.db` to `staff-room.db`.
- Renamed debug voice overrides to `STAFF_ROOM_WHISPER_CLI` and `STAFF_ROOM_WHISPER_MODEL`.
- Renamed project memory from `.agent-room/memory.md` to `.staff-room/memory.md`, retaining a read-only fallback for existing project context, and renamed managed branches from `agent-room/*` to `staff-room/*`.
- Renamed the internal provider handoff markers from `AGENT_ROOM_*` to `STAFF_ROOM_*`.
- Limited bundling to the supported NSIS target.

### Removed

- Generated redesign logs, QA captures, state files, and superseded implementation plans from the release tree.

### Release status

This is a release candidate. The `1.0.0` tag remains behind the complete human acceptance gate in [docs/PUBLIC_RELEASE_CHECKLIST.md](docs/PUBLIC_RELEASE_CHECKLIST.md).
