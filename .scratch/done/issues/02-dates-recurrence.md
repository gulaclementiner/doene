# 02: Dates + recurrence engine with parity fixtures

**What to build:** All date math and next-occurrence computation for the six recurrence types behave exactly like Gör't's, pinned by ported parity fixtures — the foundation every dated feature builds on.

**Blocked by:** 01 (Workspace scaffolding)

**Status:** ready-for-agent

- [ ] UTC date arithmetic (add/clamp/week-start) matches Gör't, including month-end clamping (Jan 31 + 1 month → Feb 28/29)
- [ ] Next-occurrence is correct for daily/weekdays/weekly/monthly/yearly/custom weekday sets with intervals, using "next on or after" semantics
- [ ] All ported parity fixtures from Gör't's date and recurrence suites are green (each reviewed at port time; any quirk/bug becomes a logged divergence)
- [ ] Property tests for recurrence invariants pass
