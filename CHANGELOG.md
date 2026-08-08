# Changelog

All notable changes to The Staff Room are recorded here. The project follows Semantic Versioning after the public `1.0.0` release.

## [1.0.0-rc.1] - 2026-08-08

### Added

- Public architecture, build, roadmap, security, contribution, and release-checklist documentation.
- MIT licensing metadata.
- A restrictive production Content Security Policy.
- Windows continuous integration for the reproducible local verification gate.
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

This is a release candidate. Public visibility, the repository rename, metadata, profile pin, and the `1.0.0` tag remain behind the explicit human gate in [docs/PUBLIC_RELEASE_CHECKLIST.md](docs/PUBLIC_RELEASE_CHECKLIST.md).
