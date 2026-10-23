# 10: First visible app — rail, All list, composer, theme toggle

**What to build:** The tracer bullet lights up: the window shows the rail with system lists and a task list; typing in the quick-add composer creates an NLP-parsed task that appears in the list with correct sorting; light/dark theme toggles and persists; everything survives restart.

**Blocked by:** 09 (Task persistence), 08 (i18n tables + labels)

**Status:** ready-for-agent

- [ ] A composer line becomes a parsed task rendered in the All list (subject, due date, tags, importance marker)
- [ ] List ordering uses the core sort rules
- [ ] Theme toggle works and persists across restarts
- [ ] Restart preserves all data
