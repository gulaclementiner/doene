# 03: NLP quick-add parser — English

**What to build:** An English quick-add line ("Buy milk tomorrow #errands !high") parses into structured task fields — dates, times, tags, importance, shortlist markers, recurrence phrases — with a literal-escape path that keeps text unparsed.

**Blocked by:** 02 (Dates + recurrence engine)

**Status:** ready-for-agent

- [ ] The English keyword set parses to the same structured output as Gör't's parser for all ported fixture cases
- [ ] Nth-of-month ("every 1st") and explicit day/month dates are handled
- [ ] The literal-escape path returns raw text unparsed
- [ ] Fuzz/property tests on the English parser pass
