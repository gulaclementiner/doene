# 22: Settings popover + language switch

**What to build:** A settings surface for week start day, due-notification time, and My Day group-heading display; an EN/SV language switch across the whole UI with OS-locale detection on first run.

**Blocked by:** 10 (First visible app), 08 (i18n tables + labels)

**Status:** ready-for-agent

- [ ] All three settings are editable, persist, and take effect (week start in date labels; notify time in scheduling)
- [ ] The language switch re-renders every string with no missing keys
- [ ] First run detects the OS locale
