# 06: View matching/sorting + search scoring with parity fixtures

**What to build:** The pure read-model rules for views and search: any/all tag matching, chained multi-level sort comparators (name/due/created/manual × asc/desc), and weighted search scores.

**Blocked by:** 02 (Dates + recurrence engine)

**Status:** ready-for-agent

- [ ] Any/all tag matching is fixture-pinned
- [ ] Chained sort levels — including null-due handling and stable tie-breaks — match Gör't's comparator behaviour
- [ ] Search scoring weights (prefix > word-start > substring > body, tag bonus) are fixture-pinned
