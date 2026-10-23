# Döne — Implementation Plan

Status: **agreed** (grilling session complete; ready to be turned into a spec).
Vocabulary: see `GLOSSARY.md`. Decisions: see `docs/adr/`.

## 1. Purpose

Döne is a clean-room reimplementation of Gör't (`.ref/gort`) in Rust + egui: a single native binary, local-first To Do app. Three goals, in order:

1. **Same user-facing functionality as Gör't** — nothing changes from the user's perspective, except bugs we find and fix (each such fix is a recorded *divergence*).
2. **Structurally more efficient** — one native process instead of Electron + Node + Chromium; no HTTP loopback; direct SQLite access. Efficiency is a steering directive, not a budget (no numeric targets).
3. **Built to be extended for a long time** — clean crate boundaries, a pure core, and a service layer designed from day one as the integration seam for future MCP and Outlook-API clients.

## 2. v1 Scope

**In:** tasks, steps (promote to task), subtasks (tree with cycle guards); tags; views (saved filters: name, emoji icon, tags, match any/all, up to 6 sort levels, include-completed toggle); system lists (My Day, Shortlisted, Important, Planned, All, Completed); My Day agenda (user-ordered groups, movable section break, completed-today pinning, midnight expiry); recurrence (daily/weekdays/weekly/monthly/yearly/custom incl. "every 1st"; completing spawns the next occurrence); natural-language quick add (EN + SV: dates, times, `#tags`, `!high`, shortlist, recurrence phrases; Escape keeps text literal); search scoring + Ctrl+K command palette; per-view sort and colour; light/dark themes; EN/SV locales; JSON export/import (our own format); trash with restore/permanent-delete; undo (Ctrl+Z) + toasts; activity log; **native OS notifications** for reminders and due-today (fires while the app runs, even with the window closed — a strict improvement over Gör't's "while the page is open"); mouse drag & drop *and* keyboard reordering/nesting of tasks.

**Out (explicit):**
- REST/HTTP API in v1 → replaced by the in-process service layer (ADR-0001). MCP and Outlook integrations are post-v1 (see §11).
- Cloud sync/sharing, attachments (also unimplemented in Gör't).
- macOS/Linux *release* targets — codebase stays cross-platform; Windows is the primary dev/test target.
- Auto-update. Installer comes at the very end (M8); v1 runs as a self-contained portable binary.
- Gör't data migration — fresh start. We do not read `gort.db`; import works with Döne's own export format only.

## 3. Guiding Principles

1. **One-way dependencies, no exceptions:** `ui → service → storage → core`; `notify` standalone. Any layer can be swapped (DB, frontend, API server) without touching the others.
2. **The core is pure.** No I/O, no UI types, deterministic. Every rule — dates, recurrence, NLP, agenda, matching, sorting, scoring, DnD zones, notification scheduling — is a side-effect-free function testable without a database or a window.
3. **Parity discipline.** Gör't's behaviour is the baseline spec. Deviations happen only for found bugs/quirks and are always recorded in `docs/divergences.md` with rationale plus a test pinning the new behaviour. Nothing is sacred, nothing is skipped.
4. **No structural shortcuts.** Crate boundaries, normalized schema, typed data across layer boundaries — even where a JSON blob or a single crate would be easier today.
5. **Efficiency by architecture, verified informally.** No budgets; M8 records observed startup/RAM/CPU next to the Electron baseline for reference.

## 4. Architecture

Six-crate workspace (agreed layout):

| Crate | Role | Depends on |
|---|---|---|
| `döne-core` | Pure domain logic: `dates`, `recurrence`, `nlp`, `agenda`, `smartlists`, `views`, `search`, `tree`, `i18n`, `labels`, notification scheduling rules. Small deliberate public API: entities + read-model builders + pure rule functions. | — |
| `döne-storage` | SQLite via rusqlite; migrations; JSON export/import. | core |
| `döne-service` | Application layer orchestrating core rules + storage: complete → spawn next occurrence, delete → trash snapshot, activity log, validation with structured error codes (`{ code, params }`, mirroring Gör't's). **The integration seam** (ADR-0001/0002): designed from day one as a foreign-process interface — explicit methods, plain data in/out, no UI types across the boundary, transport-agnostic. | core, storage |
| `döne-ui` | eframe/egui app: rail, task lists, drawer, palette, composer, layout editor, DnD glue, toasts, settings, themes, i18n strings. | service (+ core for types) |
| `döne-notify` | Notification trait + Windows backend; cross-platform stubs. | — (standalone) |
| `döne` (binary) | Composition root: data-dir resolution (`%APPDATA%/Döne`, env-overridable), wiring, main loop. | all of the above |

Future integration crates (`döne-mcp`, `döne-api`) will depend on `döne-service` only — see §11.

## 5. Data Model

**Conventions:** ids = UUIDv4 TEXT; timestamps ISO-8601 UTC; dates `YYYY-MM-DD` computed in UTC (porting Gör't's timezone-drift discipline); manual order = `position REAL` with gap-free 1..n renumbering on move.

**Schema (normalized, typed — our own, no Gör't compatibility):**

```sql
tasks(
  id TEXT PRIMARY KEY, subject TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','completed')),
  importance TEXT NOT NULL DEFAULT 'normal' CHECK (importance IN ('normal','high')),
  due_date TEXT, reminder_at TEXT,          -- 'YYYY-MM-DD' / local 'YYYY-MM-DDTHH:MM'
  body TEXT, focus_date TEXT, postponed_date TEXT,
  created_at TEXT NOT NULL, completed_at TEXT,
  position REAL NOT NULL DEFAULT 0,
  spawned_from TEXT REFERENCES tasks(id), parent_id TEXT REFERENCES tasks(id),
  -- recurrence (normalized columns; NULL type = no recurrence)
  recurrence_type TEXT CHECK (recurrence_type IN ('daily','weekdays','weekly','monthly','yearly','custom')),
  recurrence_interval INTEGER, recurrence_unit TEXT CHECK (recurrence_unit IN ('day','week','month','year')),
  recurrence_days TEXT                       -- CSV of weekdays, custom rules only
)
steps(id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      subject TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','completed')),
      position REAL NOT NULL DEFAULT 0)
views(id TEXT PRIMARY KEY, name TEXT NOT NULL, emoji TEXT,
      match_mode TEXT NOT NULL DEFAULT 'any' CHECK (match_mode IN ('any','all')),
      show_done INTEGER NOT NULL DEFAULT 0, position REAL NOT NULL DEFAULT 0)
view_tags(view_id TEXT REFERENCES views(id) ON DELETE CASCADE, tag TEXT NOT NULL, PRIMARY KEY (view_id, tag))
view_sorts(view_id TEXT NOT NULL REFERENCES views(id) ON DELETE CASCADE, level INTEGER NOT NULL,
           by TEXT NOT NULL CHECK (by IN ('name','due','created','manual')),
           dir TEXT NOT NULL DEFAULT 'asc' CHECK (dir IN ('asc','desc')), PRIMARY KEY (view_id, level))  -- max 6 levels
task_tags(task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE, tag TEXT NOT NULL, PRIMARY KEY (task_id, tag))
trash(id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('task','view')), label TEXT,
      data TEXT NOT NULL,                    -- full JSON snapshot incl. subtree/steps
      deleted_at TEXT NOT NULL)
activity(id TEXT PRIMARY KEY, task_id TEXT, at TEXT NOT NULL, action TEXT NOT NULL, detail TEXT)
settings(key TEXT PRIMARY KEY, value TEXT)   -- theme, language, weekStart, dueNotifyTime, showMyDayGroupHeadings
prefs(scope TEXT PRIMARY KEY, value TEXT)    -- 'global' | 'task:<id>' → { drawerOrder: [...] }
fired_notifications(key TEXT PRIMARY KEY, at TEXT NOT NULL)  -- dedupe set, pruned to last 500 (replaces Gör't's localStorage gort.fired)
```

**Export format:** versioned JSON `{ "version": 1, tasks[], views[], settings{}, prefs{} }` — layout defined by us; import = wipe and reload (same semantics as Gör't). The `version` field gives us a migration path for our own future formats.

## 6. Parity & Verification Strategy

- **Parity fixtures (backbone):** all ~397 of Gör't's unit test cases are ported into Rust tests against `döne-core` (and `döne-service` where the case exercises orchestration), each reviewed at port time: *intended behaviour* → keep as-is; *quirk/bug* → divergence test + entry in `docs/divergences.md`. Gör't's mutation-survivor pins are treated as high-value edge knowledge.
- **Manual e2e matrix:** Gör't's 18 Playwright spec files (119 tests) become `docs/test-matrix.md` — one section per spec file, checked off as the corresponding milestone lands; full side-by-side pass against `.ref/gort` (`npm start`) at M8.
- **Property/fuzz tests:** NLP parser and recurrence engine get property-based tests (proptest), mirroring Gör't's fast-check suite.
- **No egui UI test harness** — the manual matrix plus pure-logic coverage is the v1 verification story.

## 7. Algorithms to Port Carefully

| # | Algorithm | Reference location | Risk notes |
|---|---|---|---|
| 1 | UTC date math | `src/dates.js` | month-end clamping (Jan 31 + 1mo → Feb 28/29); `weekStartOf` honours Mon/Sun setting |
| 2 | Recurrence next-occurrence | `src/recurrence.js` | monthly/yearly keep original day with clamping; custom weekday sets advance strictly after anchor; "next on or after" spawn semantics |
| 3 | NLP quick-add parser | `src/nlp.js`, `src/quickadd.js` | EN+SV keywords; nth-of-month ("every 1st"); `suggestQuickAdd`; literal-escape path |
| 4 | My Day agenda | `src/agenda.js`, `src/agenda-order.js` | group ordering with user-movable section break; midnight expiry via focus date; completed-today pinning in completion order |
| 5 | Multi-level sort comparator | `src/view-rules.js` | chained levels, manual = stored position, null-due handling, stable tie-breaks |
| 6 | Search scoring | `src/search.js` | prefix 100 / word-start 80 / substring 60 / body 20 / tag +40 |
| 7 | Tree cycle guards | `src/tree.js`, `public/task-drop.js` | reject nesting under self/descendants (depth-limited walk) |
| 8 | DnD zone geometry | `public/task-drop.js` | zone = bottom of group above → top quarter (0.25) of card below; deepest list wins; place/nest/unnest results |
| 9 | Position renumbering | `src/store.js`, `public/rail-order.js` | gap-free 1..n, return only changed rows |
| 10 | Notification scheduling | `public/notify.js` | pure function: reminder grace window (24 h), due-today at/after `dueNotifyTime` once/day, per-day dedupe keys |
| 11 | Spawn-on-complete | `src/store.js` | completing a recurring task creates the next occurrence linked by `spawned_from`; uncompleting removes the spawn |

## 8. UI Plan (`döne-ui`)

- **Structure:** eframe app with a single `AppState` (service handle + UI state: active list/view, selection, open drawer, palette, toasts, theme). Modules mirror Gör't's `public/` split: rail, tasklist rendering, drawer, palette, composer, layout editor (My Day groups + drawer sections), DnD glue.
- **Drag & drop is first-class** (a primary interaction, not polish): the pure zone/canNest/dropResult logic lives in `döne-core` and is parity-tested *before* any mouse glue; egui hit-testing implements it on top. Keyboard reordering (Alt+arrows: move/nest/unnest) ships alongside as the guaranteed equivalent.
- **i18n:** locale files compiled into the binary — flat dotted keys, same key set as Gör't's `locales/{en,sv}.js` (~304 keys each), `tr(key, params)` with EN fallback and `{var}` interpolation. Language stored in settings; first run detects from the OS locale (native adaptation of Gör't's Accept-Language detection — recorded as a divergence-note, not a behaviour change).
- **Themes:** light/dark egui visual styles, persisted to `settings.theme`, toggle in rail.
- **Keyboard shortcuts:** Ctrl+K palette (system lists/views/commands/top tasks via search scoring), Ctrl+N focuses quick add, Ctrl+Z undoes outside text inputs, Escape in quick add keeps text literal.

## 9. Notifications

- Scheduling rules ported as a pure function into `döne-core` (algorithm #10); the UI process ticks every 60 s and fires OS toasts through `döne-notify` (Windows backend first).
- Dedupe keys persist in `fired_notifications` (last 500), replacing Gör't's browser localStorage.
- Intentional improvement over Gör't: notifications fire while the app runs even with the window closed/minimized — recorded in `docs/divergences.md`.

## 10. Milestones

Every milestone ends in a working binary.

| M | Contents | Exit criteria |
|---|---|---|
| **M0** | Workspace scaffolding: six crates as stubs, tooling (fmt/clippy/test), data-dir resolution in the binary | `cargo fmt --check`, `clippy`, `test` all green; empty window launches |
| **M1** | `döne-core` complete + parity fixtures | All ported fixture suites green (dates, recurrence, NLP EN+SV, agenda, smartlists, view-rules, search, tree, i18n, labels, notify rules); property/fuzz tests in place; `docs/divergences.md` started |
| **M2** | `döne-storage` + `döne-service` | Schema + migrations; service operations mirror Gör't's 20 endpoint semantics (tasks CRUD/quickadd, views CRUD+order, trash list/restore/purge, export/import, settings, prefs, search) with structured errors; integration tests against temp DBs |
| **M3** | First visible window: rail (system lists + views + Trash + theme toggle), task-list rendering for system lists, light/dark | `docs/test-matrix.md` created; corresponding matrix sections checked off side-by-side with `.ref/gort` |
| **M4** | Task drawer (notes, due, reminder, recurrence editor, importance, steps w/ promote-to-task, subtasks), undo stack + toasts | Matrix sections for tasks/drawer/subtasks/sections/trash-undo checked off |
| **M5** | Quick-add composer with NLP + literal escape; tag input; view create/edit form (emoji, tags, match any/all, ≤6 sort levels, showDone); Ctrl+K palette + search | Matrix sections for quickadd-palette/custom-views/tags/selection checked off |
| **M6** | My Day agenda (groups, movable section break, completed-today pinning, midnight expiry), recurrence spawn UX, **mouse DnD + keyboard reordering/nesting** with zone logic and cycle guards | Matrix sections for myday-agenda/myday-layout/task-dnd/layout checked off; DnD verified against reference behaviour |
| **M7** | OS notifications (reminders + due-today) with dedupe; EN/SV language switch across the UI; settings popover (week start, due-notify time, My Day group headings); export/import UI | Matrix sections for preferences/i18n-sv/design checked off |
| **M8** | Polish + installer | Full manual matrix pass complete; efficiency observations recorded (startup/RAM/CPU vs Electron baseline, informational only); Windows installer produced as the final artifact |

## 11. Deferred — Explicit Post-v1 Branches

Not silently assumed; each is a known future decision:

- **`döne-mcp`** — MCP server for AI agents (stdio JSON-RPC), forwarding tool calls to the live app through `döne-service`.
- **`döne-api`** — local API for integrations like Outlook macros writing tasks; transport choice (named pipe vs localhost HTTP) decided then.
- **Not-running behaviour** — what external clients do when Döne isn't running (start it? refuse?).
- **macOS/Linux release targets** — buildable from day one, released later if wanted.
- **Gör't legacy export import** — only if a migration need ever appears.
- **Auto-update** — out of scope.

## 12. Documentation & Process Map

| Artifact | Location | Status |
|---|---|---|
| Glossary (vocabulary) | `GLOSSARY.md` | maintained as terms resolve |
| Architecture decisions | `docs/adr/0001-…`, `docs/adr/0002-…` | recorded |
| This plan | `docs/plan.md` | agreed |
| Divergence log | `docs/divergences.md` | created at M1 |
| Manual e2e matrix | `docs/test-matrix.md` | created at M3 |
| Spec + issues | `.scratch/done/` — one issue file per milestone (M0–M8) | when this plan is turned into a spec |

`.ref/gort` stays in the project folder (gitignored, read-only) as the side-by-side reference for the entire development: `npm start` there whenever behaviour needs comparing.
