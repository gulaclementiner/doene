# 08: i18n string tables + label formatting with parity fixtures

**What to build:** The EN and SV locale tables (~304 keys each) compiled into the binary as plain-text data files (one file per language — adding a future language is dropping in one new file, no code change), lookup with English fallback and `{var}` interpolation, plus date/importance label formatting. Translation gaps must be machine-discoverable so bulk translation can run in external tools.

**Blocked by:** 01 (Workspace scaffolding)

**Status:** ready-for-agent

- [ ] Locale tables are plain-text data (not hand-written Rust syntax); a new language can be added by adding one data file without touching lookup code
- [ ] Key set is identical to Gör't's EN/SV tables; missing-key behaviour fixture-checked in both directions
- [ ] A test asserts key-set parity per locale against English, failing with the exact list of missing keys (the machine-readable translation gap report)
- [ ] A test asserts `{var}` placeholder sets match across locales for every key
- [ ] Interpolation and English fallback behave per Gör't fixtures
- [ ] Label formatting (dates with week-start, importance markers) is fixture-pinned
