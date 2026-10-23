# 16: Views CRUD + rail rendering

**What to build:** Users create, edit, and delete saved views (name, emoji icon, tags with any/all matching, up to six chained sort levels, include-completed toggle); views appear in the rail with colour, are reorderable by dragging, and apply their own matching and sorting.

**Blocked by:** 15 (Tags), 06 (View rules + search scoring)

**Status:** ready-for-agent

- [ ] The view form validates and persists all fields, including up to six chained sort levels
- [ ] Views render in the rail with emoji and colour; dragging reorders them with gap-free positions
- [ ] Opening a view shows exactly the tasks its rules match, sorted per its configuration
