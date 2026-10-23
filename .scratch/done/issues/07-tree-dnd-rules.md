# 07: Tree guards + DnD zone rules with parity fixtures

**What to build:** The pure structural rules for the task tree and drag & drop: cycle detection, drop-zone geometry, and place/nest/unnest outcomes — testable before any mouse glue exists.

**Blocked by:** 01 (Workspace scaffolding)

**Status:** ready-for-agent

- [ ] Nesting a task under itself or its own descendant is rejected (cycle guard), per Gör't fixtures
- [ ] Drop-zone geometry (bottom of the group above through the top quarter of the card below; deepest list wins) is fixture-pinned
- [ ] Place/nest/unnest drop results match Gör't for all ported cases
