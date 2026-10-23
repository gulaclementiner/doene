# 01: Workspace scaffolding + entities + tooling

**What to build:** The Döne workspace exists and runs: six crates with strictly one-way dependencies, core entity types defined in the pure core, formatting/lint/test tooling green, and an empty egui window that launches and resolves the data directory.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `cargo fmt --check`, clippy, and `cargo test` all pass on a clean checkout
- [ ] Empty app window opens and closes cleanly; the data directory is resolved (env override honoured) and created
- [ ] Core entity types (task, step, view, tag, recurrence rule, trash entry) live in `döne-core` with no I/O or UI dependencies
- [ ] Dependency rule holds: ui → service → storage → core, notify standalone
