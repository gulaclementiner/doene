# 08: i18n string tables + label formatting with parity fixtures

**What to build:** The EN and SV locale tables (~304 keys each) compiled into the binary, lookup with English fallback and `{var}` interpolation, plus date/importance label formatting.

**Blocked by:** 01 (Workspace scaffolding)

**Status:** ready-for-agent

- [ ] Key set is identical to Gör't's EN/SV tables; missing-key behaviour fixture-checked in both directions
- [ ] Interpolation and English fallback behave per Gör't fixtures
- [ ] Label formatting (dates with week-start, importance markers) is fixture-pinned
