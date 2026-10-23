# 14: Steps + subtasks

**What to build:** Tasks can hold checklist steps that tick off and promote to full tasks; tasks nest as subtasks with cycle guards enforced, so the tree is always renderable.

**Blocked by:** 13 (Task drawer — fields), 07 (Tree guards + DnD zone rules)

**Status:** ready-for-agent

- [ ] Steps can be added, ticked off, and deleted; promote-to-task creates a real task
- [ ] Subtask nesting renders the tree; invalid nesting is rejected by the cycle guard with feedback
- [ ] Deleting a parent snapshots the whole subtree into Trash (with ticket 12's snapshot)
