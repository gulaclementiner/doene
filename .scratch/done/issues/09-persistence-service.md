# 09: Task persistence — schema, storage, service operations

**What to build:** The data layer works end-to-end without a UI: the normalized SQLite schema (tasks with recurrence columns, steps, task_tags, settings, activity), migrations, and service operations for create/patch/delete/quick-add/search carrying Gör't's structured error codes. Every mutation is logged to the activity log.

**Blocked by:** 03 (NLP — English), 06 (View rules + search scoring)

**Status:** ready-for-agent

- [ ] Schema and migrations apply cleanly; conventions hold (UUIDv4 TEXT ids, ISO-8601 UTC timestamps, YYYY-MM-DD dates computed in UTC)
- [ ] Service operations mirror Gör't's task-endpoint semantics, including quick-add via the NLP parser and search via core scoring
- [ ] Structured `{code, params}` errors match Gör't's error-code suite
- [ ] Integration tests against a temporary database are green; an activity row is written on every mutation
