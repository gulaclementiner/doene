# 12: Trash + restore/permanent delete + undo/toasts

**What to build:** Deleting a task moves its full subtree to Trash as a snapshot; it can be restored or permanently deleted there; Ctrl+Z undoes the last action with a toast confirmation. The undo mechanism is generic at the service level — later tickets just register their operations.

**Blocked by:** 11 (Complete/uncomplete + spawn-on-complete)

**Status:** ready-for-agent

- [ ] Delete snapshots the task plus subtasks and steps into Trash; lists update immediately
- [ ] Restore recreates the full subtree; permanent delete removes it for good
- [ ] Ctrl+Z reverses the last mutation with a toast; undo works across operation types
