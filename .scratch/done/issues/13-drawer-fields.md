# 13: Task drawer — fields

**What to build:** Selecting a task opens its detail drawer with editable notes, due date, reminder time, importance, and a recurrence editor (all six types including custom weekday sets and intervals), plus the task's activity view. Per-task drawer section ordering is stored in prefs.

**Blocked by:** 12 (Trash + undo/toasts), 08 (i18n tables + labels)

**Status:** ready-for-agent

- [ ] All fields are editable and persist via service operations
- [ ] The recurrence editor exposes all six types with interval and custom weekday sets
- [ ] The activity view shows the task's log entries
- [ ] Drawer section order persists per task (prefs)
