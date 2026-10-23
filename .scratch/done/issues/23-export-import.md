# 23: Export/import

**What to build:** All data exports as a versioned JSON file and imports back with wipe-and-reload semantics, both reachable from the UI.

**Blocked by:** 14 (Steps + subtasks), 16 (Views CRUD), 21 (OS notifications)

**Status:** ready-for-agent

- [ ] Export includes tasks (steps, tags, recurrence), views, settings, prefs, trash, and activity; a version field is present
- [ ] Import wipes and reloads; app state matches the file afterwards
- [ ] Both operations are reachable from the UI; a round-trip integration test is green
