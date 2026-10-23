# 24: Manual matrix pass + fixes

**What to build:** The full side-by-side verification against Gör't: every section of the manual e2e matrix (converted from Gör't's Playwright specs) checked off with both apps running, discrepancies fixed or logged as divergences. Work it section by section — this ticket is a verification sweep, not a feature build.

**Blocked by:** 17 (Palette + search), 18 (My Day UI), 20 (Mouse drag & drop), 22 (Settings + language), 23 (Export/import)

**Status:** ready-for-agent

- [ ] Every section of the manual matrix is checked off, or its discrepancy is logged as a divergence with rationale
- [ ] Any fix lands with a regression test at the appropriate seam (core fixtures or service integration tests)
- [ ] The divergence log is complete and consistent with the pinning tests
