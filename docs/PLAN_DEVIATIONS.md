# Plan deviations

- Step 5 | Capability handshake on first provider/version sight | Retained the declared capability table without an automatic minimal turn because the current synchronous provider discovery path has no project/worktree authority and must not launch a billable or write-capable CLI at desktop startup; connection tests remain the explicit live verification path.

- Step 1 | `lib.rs:2774–3178` | Deleted the PTY subsystem by symbol boundary through line 3177 and restored the adjacent `persist_message` Clippy allowance | The specified range clipped a required attribute belonging to the following function.
- Step 1 | `src/coordination.test.ts` whole file | Reduced the file to four surviving participant-selection tests | The file also protects product behaviour used by later Ask/Quick Edit filtering.
- Step 1 | Remove `Read` and `AsyncWriteExt` with the PTY code | Retained both imports | They are used by the one-shot process output and stdin paths that remain in v1.
- Step 4 | `commands/mod.rs` and `db/migrations.rs` | Implemented the project commands and v1-to-v2 migration in `src-tauri/src/lib.rs` | This codebase owns Tauri commands and SQLite migrations in that file, so creating parallel modules would add unused architecture.
# Step 3 — Plan said `db/schema.rs`; the schema and additive migrations are owned by `src-tauri/src/lib.rs` in this codebase, so the equivalent columns were added there to preserve the existing migration path.
