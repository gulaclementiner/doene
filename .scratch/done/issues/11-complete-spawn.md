# 11: Complete/uncomplete + spawn-on-complete

**What to build:** Ticking a task completes it; completing a recurring task automatically spawns the next occurrence linked back to its parent; uncompleting removes the spawn and restores the prior state.

**Blocked by:** 10 (First visible app)

**Status:** ready-for-agent

- [ ] Complete/uncomplete updates status and completion time, and is logged to the activity log
- [ ] Completing a recurring task spawns the next occurrence with the correct due date (core engine) and a spawn link
- [ ] Uncompleting a spawned occurrence restores the prior state per Gör't semantics
