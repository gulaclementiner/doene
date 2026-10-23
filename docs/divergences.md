# Divergence log

Intentional behavioural differences from Gör't. Every entry records a found
bug/quirk that we fixed, why, and the test pinning the new behaviour.
Parity discipline: Gör't's behaviour is the baseline; nothing diverges without
an entry here (docs/plan.md §3).

**Entry format:** *what diverged* → *why* → *pinning test*.

---

## D1 — Impossible calendar dates are rejected, not normalised

- **What diverged:** Gör't's `dates.parse` accepts impossible calendar dates
  and silently normalises them via JS `Date` rollover (e.g. `'2025-02-30'`
  parses to `2025-03-02`, so `addDays('2025-02-30', 1)` → `'2025-03-03'`).
  Döne's `Date::parse` rejects them with `InvalidDate`.
- **Why:** silent normalisation masks bad data — a stored due date of Feb 30
  would drift to Mar 2 on every arithmetic call, invisibly. With a typed
  `Date`, the invalid state is unrepresentable (docs/plan.md §4: "no structural
  shortcuts"). No Gör't fixture exercises this path: every date in Gör't's
  suites is a valid calendar day (property generators produce dates via JS
  `Date` normalisation, NLP validates before use).
- **Pinning test:** `doene-core::dates` unit tests —
  `rejects_day_beyond_month_end`, `accepts_leap_day_in_leap_year`,
  `rejects_leap_day_in_century_non_leap_year`, `rejects_malformed_input`
  (`crates/doene-core/src/dates.rs`).

## D2 — No `error.recurrenceInvalid` at the next-occurrence level

- **What diverged:** Gör't's `recurrence.nextDate` throws coded
  `error.recurrenceInvalid` ("bad recurrence") when handed a rule with an
  unknown type. Döne's `next_date` is total over the closed
  `RecurrenceType` enum and has no such error; unknown types are rejected
  earlier by `validate` with `error.recurrenceTypeInvalid`. The
  `error.recurrenceInvalid` code therefore does not exist in Döne.
- **Why:** a rule that failed validation cannot exist as a typed value, so the
  second check is dead code (docs/plan.md §4). Gör't needs it because plain JS
  objects can carry any `type`; the typed reimplementation makes the invalid
  state unrepresentable instead.
- **Pinning test:**
  `crates/doene-core/tests/error_codes_recurrence.rs::unknown_type_rejected_by_validate`
  (ported from Gör't `test/module-error-codes.test.js`, "unknown recurrence in
  nextDate carries code").

## D3 — Non-numeric weekday tokens are dropped, not coerced

- **What diverged:** Gör't's `validate` maps custom-weekday tokens through JS
  `Number()`, which coerces blank and exotic strings: `""` → 0 (Sunday),
  `"1e0"` → 1 (Monday), `" 3 "` → 3. Döne's `validate` accepts only plain
  integer strings (`"0"`…`"6"`, surrounding whitespace tolerated) and drops
  everything else.
- **Why:** the JS coercion is a silent data-corruption quirk — one empty
  string in a weekday set silently selects Sunday. Döne's raw shape is typed
  (`Vec<String>`), so strict parsing is natural; dropping unparseable tokens
  matches Gör't for genuinely invalid values (e.g. `"9"`, `"-1"`), and the
  only differences are on inputs no Gör't code path ever produces (NLP emits
  bare digit strings).
- **Pinning test:**
  `crates/doene-core/tests/recurrence_mutants.rs::recurrence_validate` —
  "non-numeric weekday tokens are dropped" case (`["", "1e0", "3"]` → only
  Wednesday survives).
