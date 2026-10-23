# 21: OS notifications

**What to build:** Reminders and the once-daily due-today notification arrive as native OS toasts while the app runs — even with the window closed or minimized — at most once per task per day.

**Blocked by:** 10 (First visible app)

**Status:** ready-for-agent

- [ ] Per-task reminders fire within the grace window; due-today fires at/after the configured time
- [ ] Dedupe is persisted (fired set, pruned to the last 500 keys) — no repeats across restarts
- [ ] The Windows backend delivers native toasts; other platforms compile via trait stubs
