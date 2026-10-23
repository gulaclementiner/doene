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

## D4 — Pinned CLDR data instead of runtime `Intl` (i18n + labels)

- **What diverged:** Gör't reads three things from the JS engine's ICU at
  runtime: short month names via `Intl.DateTimeFormat(locale, { day, month:
  'short' })` (`src/labels.js`), number grouping via
  `Intl.NumberFormat(displayTag(locale))` (`src/i18n.js`), and plural
  categories via `Intl.PluralRules(tag)` with a fallback to the default
  locale (`src/i18n.js`). Rust has no built-in ICU. Döne pins the exact CLDR
  values Gör't's engine produces for the shipped locales instead of computing
  them: short month names for `en` and `sv` in
  `doene-core::labels::month_short`; number grouping for `en` (`,` groups,
  `.` decimal) and `sv` (NBSP groups, `,` decimal) in
  `doene-core::i18n::format_number`; plural rules for `en`/`sv` (`one` only
  for exactly 1, else `other`) plus `pl` as a fixture reference (`one` = 1;
  `few` = integers ending 2–4 except 12–14; `many` = other integers; `other`
  = fractions) in `doene-core::i18n::plural_category`.
- **Why:** the values are pinned from the same CLDR data Gör't's Node runtime
  uses, so output is byte-identical for en/sv. Pinning keeps `doene-core`
  dependency-free (no `icu_*` crates) and makes locale formatting
  deterministic across platforms. Unknown locales fall back to English
  values/rules — matching Gör't's own fallback-to-default-locale behaviour
  for unusable tags.
- **Pinning tests:** `labels::tests::short_date_day_first_with_the_locale_month_en_and_sv`,
  `labels::tests::short_date_a_non_current_month_uses_its_own_month_name`,
  `i18n::tests::format_number_groups_digits_per_locale`,
  `i18n::tests::plural_category_rules_for_shipped_and_fixture_locales`.

## D5 — Language detection input (note, not a behaviour change)

- **What diverged:** Gör't detects the language from an `Accept-Language`
  header; Döne is a desktop app and detects it from the OS locale on first
  run (docs/plan.md §i18n). The detected choice is then stored in settings.
- **Why:** there is no HTTP client/server boundary in a desktop app. The
  ranking logic itself (q-value ordering, base-language matching, English
  fallback) is ported unchanged so the behaviour stays fixture-pinned.
- **Pinning tests:** `i18n::tests::detect_language_picks_the_best_supported_language_by_q_value`,
  `i18n::tests::detect_language_handles_missing_or_empty_input`.

## D6 — Impossible ISO dates in quick-add text stay in the subject

- **What diverged:** Gör't's NLP accepts an explicit `YYYY-MM-DD` token as a due
  date whenever JS `Date.parse` does not return NaN. V8 rolls impossible
  calendar days over (`Date.parse('2025-02-31')` → Mar 3), so `'x 2025-02-31'`
  gets `due_date: '2025-02-31'` in Gör't — a due date that later arithmetic
  silently drifts (D1). Döne's NLP validates the token through
  `Date::parse`, which rejects impossible days, so the token is not recognised
  and stays in the subject.
- **Why:** this is D1 applied at the NLP seam: an impossible due date must be
  unrepresentable, and the parser is where user input becomes a typed `Date`.
  Accepting `'2025-02-31'` would store a value that drifts on every read.
- **Pinning test:**
  `crates/doene-core/tests/nlp_search.rs::an_impossible_iso_date_stays_in_the_subject`.
