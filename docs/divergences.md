# Divergences from Gör't

Gör't's behaviour is the baseline spec (docs/plan.md §3). Every entry here records a
deliberate deviation: what diverged, why, and the test that pins the new behaviour.
No user-visible behaviour changes without an entry in this file.

## 1. Pinned CLDR data instead of runtime `Intl` (i18n + labels)

**What diverged.** Gör't reads three things from the JS engine's ICU at runtime:

- short month names via `Intl.DateTimeFormat(locale, { day, month: 'short' })` (`src/labels.js`),
- number grouping via `Intl.NumberFormat(displayTag(locale))` (`src/i18n.js`),
- plural categories via `Intl.PluralRules(tag)` with a fallback to the default locale (`src/i18n.js`).

Rust has no built-in ICU. Döne pins the exact CLDR values Gör't's engine produces for the
shipped locales instead of computing them:

- short month names for `en` and `sv` in `doene-core::labels::month_short`,
- number grouping for `en` (`,` groups, `.` decimal) and `sv` (NBSP groups, `,` decimal)
  in `doene-core::i18n::format_number`,
- plural rules for `en`/`sv` (`one` only for exactly 1, else `other`) plus `pl` as a
  fixture reference (`one` = 1; `few` = integers ending 2–4 except 12–14; `many` = other
  integers; `other` = fractions) in `doene-core::i18n::plural_category`.

**Why.** The values are pinned from the same CLDR data Gör't's Node runtime uses, so output is
byte-identical for en/sv. Pinning keeps `doene-core` dependency-free (no `icu_*` crates) and
makes locale formatting deterministic across platforms. Unknown locales fall back to English
values/rules — matching Gör't's own fallback-to-default-locale behaviour for unusable tags.

**Pinning tests.** `labels::tests::short_date_day_first_with_the_locale_month_en_and_sv`,
`labels::tests::short_date_a_non_current_month_uses_its_own_month_name`,
`i18n::tests::format_number_groups_digits_per_locale`,
`i18n::tests::plural_category_rules_for_shipped_and_fixture_locales`.

## 2. Language detection input (note, not a behaviour change)

**What diverged.** Gör't detects the language from an `Accept-Language` header; Döne is a
desktop app and detects it from the OS locale on first run (docs/plan.md §i18n). The detected
choice is then stored in settings.

**Why.** There is no HTTP client/server boundary in a desktop app. The ranking logic itself
(q-value ordering, base-language matching, English fallback) is ported unchanged so the
behaviour stays fixture-pinned.

**Pinning tests.** `i18n::tests::detect_language_picks_the_best_supported_language_by_q_value`,
`i18n::tests::detect_language_handles_missing_or_empty_input`.
