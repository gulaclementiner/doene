# Spec: Döne — native reimplementation of Gör't

Status: ready-for-agent

## Problem Statement

Gör't is a web app wrapped in Electron: every session carries a full Chromium plus Node runtime, reminders only fire while the browser page is open, and its architecture (browser UI talking to a localhost HTTP server) puts a network boundary between the interface and the data. The user wants the exact same To Do experience — tasks, steps, subtasks, tags, views, My Day, recurrence, natural-language quick add in English and Swedish, search palette, drag & drop, themes, export/import — as a lightweight native application that stays responsive, notifies even when the window is closed, and can grow new capabilities (AI-agent integration via MCP, Outlook writing tasks through a macro) without architectural rework.

## Solution

Döne: a single self-contained native binary (Rust + egui) that reproduces Gör't's user-facing behaviour one-for-one — except for bugs found during the port, each recorded as an explicit divergence with rationale and a pinning test. Structurally it is more efficient: one process, no HTTP loopback, direct SQLite access. All application operations live in an in-process service layer designed from day one as the integration seam: future MCP and Outlook clients will be thin crates talking to the *running* app through that layer, never touching the database directly. Development proceeds in nine milestones (M0–M8), each ending in a working binary, with Gör't's test suite ported as reviewed parity fixtures and its e2e specs converted into a manual side-by-side matrix.

## User Stories

1. As a user, I want to create tasks from a quick-add field using natural language ("Buy milk tomorrow #errands !high"), so that capturing an idea takes one line and no form-filling.
2. As a user, I want the quick-add parser to understand dates (today/tomorrow/next week/explicit day-month), times, `#tags`, `!high` importance, shortlist markers, and recurrence phrases in both English and Swedish, so that my language habits just work.
3. As a user, I want pressing Escape in the quick-add field to keep my text literal, so that I can enter text containing keywords without it being parsed away.
4. As a user, I want tasks displayed as a list showing subject, due date, tags, importance marker, and completion state, so that I can scan my workload at a glance.
5. As a user, I want to complete and uncomplete tasks, so that I can track progress.
6. As a user, I want completing a recurring task to automatically spawn the next occurrence (linked back to its parent), so that recurring work never falls through the cracks.
7. As a user, I want uncompleting a spawned occurrence to remove it and restore the prior state, so that undoing a completion is clean.
8. As a user, I want to edit a task's subject, notes, due date, reminder time, importance, and recurrence from its detail drawer, so that tasks can be refined as details emerge.
9. As a user, I want checklist steps inside a task that I can tick off, so that I can break work into parts and see them close out.
10. As a user, I want to promote a step to a full task, so that a sub-piece can grow into its own task with notes, dates, and recurrence.
11. As a user, I want subtasks (nesting tasks under other tasks), so that related work is structured hierarchically.
12. As a user, I want cycle protection when nesting, so that I can never make a task its own ancestor and break the tree.
13. As a user, I want deleting a task to move it to Trash with full snapshot (including subtasks and steps), so that deletion is safe.
14. As a user, I want to restore tasks from Trash or permanently delete them there, so that I control what comes back and what is gone for good.
15. As a user, I want undo (Ctrl+Z) with a toast confirmation, so that accidental actions are recoverable in one keystroke.
16. As a user, I want to add `#tags` to tasks, so that I can categorize work flexibly without fixed folders.
17. As a user, I want to create saved views filtered by tags with any/all matching, each with a name and emoji icon, so that I can build my own lenses on the work.
18. As a user, I want per-view sort configuration with up to six chained levels (name/due/created/manual, ascending or descending), so that each view presents tasks in exactly the order I care about.
19. As a user, I want an include-completed toggle on views, so that I can review finished work inside a view when I choose.
20. As a user, I want to assign colours to my views and reorder them in the rail by dragging, so that my most-used lenses are recognizable and within reach.
21. As a user, I want My Day — a computed agenda for today with "Do first" and "Continue with" sections split by a movable section break — so that I can plan the day deliberately.
22. As a user, I want to group tasks in My Day and reorder those groups, so that my day has structure I control.
23. As a user, I want tasks completed today pinned at the bottom of My Day in completion order, so that I see progress without cluttering the active list.
24. As a user, I want My Day items to expire at midnight, so that yesterday's plan does not bleed into today.
25. As a user, I want the system lists Shortlisted (focus date is today), Important, Planned (has a due date), All, and Completed, so that standard lenses exist without any configuration.
26. As a user, I want a Ctrl+K command palette with weighted search over tasks (prefix > word-start > substring > body, tag bonus), so that I can jump to any task in two keystrokes.
27. As a user, I want the palette to also list system lists, views, and commands, so that it is my universal entry point into the app.
28. As a user, I want to drag tasks to reorder them within a list, so that manual order reflects my priorities.
29. As a user, I want to drag a task onto another to nest or unnest it, with visible drop zones and cycle guards, so that I can restructure the tree with my mouse.
30. As a user, I want keyboard reordering and nesting (Alt+arrows) as a full equivalent of mouse drag & drop, so that I can stay on the keyboard.
31. As a user, I want recurrence types daily/weekdays/weekly/monthly/yearly/custom weekday sets with intervals ("every 2 weeks", "every 1st of the month"), so that recurring work is expressed precisely.
32. As a user, I want monthly and yearly recurrences to clamp at month end (Jan 31 → Feb 28), so that generated dates are always valid.
33. As a user, I want per-task reminders delivered as native OS notifications, so that I am reminded even when the window is closed or minimized.
34. As a user, I want a once-daily "due today" notification at my chosen time, so that I see today's deadlines without opening the app.
35. As a user, I want each reminder/due notification to fire at most once per task per day, so that I am never spammed.
36. As a user, I want light and dark themes with persistence, so that the app fits my environment.
37. As a user, I want the interface in English or Swedish, switchable in settings and detected from my OS locale on first run, so that I can work in my language.
38. As a user, I want settings for week start day, due-notification time, and My Day group-heading display, so that the app matches my conventions.
39. As a user, I want to export all my data as a versioned JSON file, so that I can back up or move machines.
40. As a user, I want to import a Döne JSON export (wipe and reload), so that I can restore my data in one step.
41. As a user, I want an activity log recording what changed on tasks, so that I have a history trail.

## Implementation Decisions

- **Workspace layout.** Six crates with strictly one-way dependencies: `döne-core` (pure domain logic) ← `döne-storage` (SQLite via rusqlite, migrations, export/import) ← `döne-service` (application layer) ← `döne-ui` (eframe/egui app); plus standalone `döne-notify` (notification trait + Windows backend, cross-platform stubs) and the `döne` binary as composition root (data-dir resolution, wiring, main loop). No dependency may point "up".
- **Pure core.** `döne-core` contains no I/O and no UI types: internal modules for dates, recurrence, NLP parsing, agenda/My Day computation, system-list rules, view matching/sorting, search scoring, tree cycle guards, DnD zone rules, i18n string tables, label formatting, and notification scheduling. Its public API is deliberately small: entities, read-model builders, and pure rule functions.
- **Service layer as the integration seam (ADR-0001).** No HTTP server in v1. The ~20 operations Gör't exposes over REST become methods on `döne-service`, preserving Gör't's operation semantics and structured error shape (`{ code, params }`). Orchestration rules live here: completing a recurring task spawns the next occurrence linked by a `spawned_from` reference; deleting snapshots the full subtree into Trash; every mutation appends to the activity log.
- **Foreign-process-ready API (ADR-0002).** The service API is designed from day one as if a foreign process will call it: explicit methods, plain data in and out, no UI types across the boundary, transport-agnostic. Future `döne-mcp` and `döne-api` crates depend on `döne-service` only and talk to the *running* app; no integration ever opens the database directly (the GUI owns in-memory state — direct DB writes would be clobbered).
- **Storage schema.** Own normalized, typed SQLite schema — no Gör't file compatibility. Conventions: UUIDv4 TEXT ids; ISO-8601 UTC timestamps; `YYYY-MM-DD` dates computed in UTC (porting Gör't's timezone-drift discipline); manual order as a real-valued position with gap-free 1..n renumbering on move. Tables: tasks (with recurrence normalized into columns — type/interval/unit/days — plus due, reminder, focus date, postponed date, importance, status, parent and spawn links), steps, views + view_tags + view_sorts (max six levels), task_tags, trash (kind + full JSON snapshot incl. subtree/steps), activity, settings (key/value), prefs (per-scope drawer ordering), fired_notifications (dedupe set pruned to the last 500 keys, replacing Gör't's browser-localStorage dedupe).
- **Export format.** Versioned JSON (`version: 1`) containing tasks, views, settings, and prefs; layout defined by Döne. Import = wipe and reload, same semantics as Gör't. The version field is the migration path for our own future formats.
- **Data location.** `%APPDATA%/Döne`, overridable via environment variable; resolved in the binary composition root.
- **UI structure.** Single eframe app with one `AppState` (service handle + UI state: active list/view, selection, open drawer, palette, toasts, theme). Modules mirror Gör't's front-end split: rail, task-list rendering, drawer, palette, composer, layout editor (My Day groups and drawer sections), DnD glue.
- **Drag & drop is first-class.** The pure zone/canNest/dropResult rules (drop zone = bottom of the group above through the top quarter of the card below; deepest list wins; place/nest/unnest results) live in `döne-core` and are parity-tested before any mouse glue exists; egui hit-testing implements them on top. Keyboard reordering (Alt+arrows: move/nest/unnest) ships alongside as the guaranteed equivalent. DnD is flagged as the top UI risk.
- **i18n.** Locale data compiled into the binary as plain-text files, one per language — flat dotted keys, same key set as Gör't's EN/SV locale tables (~304 keys each), lookup with English fallback and `{var}` interpolation. Adding a language is dropping in one new data file, no code change; the English fallback makes partially translated locales safe to ship from day one. Key-set parity and placeholder-parity tests make translation gaps machine-discoverable — a failing test lists exactly which keys are missing per locale — so bulk translation can run in any external tool (spreadsheet, Crowdin, …) and be verified by `cargo test`. Language stored in settings; first run detects from the OS locale (native adaptation of Gör't's Accept-Language detection).
- **Themes.** Light/dark egui visual styles, persisted in settings, toggleable from the rail.
- **Notifications.** Scheduling rules are a pure function in `döne-core` (reminder grace window of 24 h; due-today at/after the configured time, once per day; per-day dedupe keys). The UI process ticks every 60 s and fires OS toasts through `döne-notify`. Intentional improvement over Gör't: notifications fire while the app runs even with the window closed — recorded as a divergence.
- **Milestones.** M0 scaffolding → M1 core + parity fixtures → M2 storage + service → M3 first visible window (rail, system lists, themes) → M4 drawer/steps/subtasks/undo → M5 quick-add/tags/views/palette → M6 My Day + recurrence spawn UX + DnD (mouse and keyboard) → M7 notifications/i18n/settings/export-import UI → M8 polish + installer. Every milestone ends in a working binary; the Windows installer is the final artifact, v1 otherwise ships as a portable binary.

## Testing Decisions

- **What makes a good test here:** tests assert observable behaviour of pure functions and the service API — parsed fields from an input line, computed next-occurrence dates, agenda shape and ordering, sort results, search scores, zone/drop outcomes, service-side effects (spawned rows, trash snapshots, activity entries) — never internal structure. Parity fixtures pin Gör't's observable outputs as the baseline.
- **Seam 1 — `döne-core` (pure logic).** All ~397 of Gör't's unit test cases are ported as parity fixtures, each reviewed at port time: intended behaviour is kept as-is; a quirk or bug becomes a divergence test asserting the fixed behaviour plus an entry in the divergence log with rationale. Gör't's mutation-survivor pins are treated as high-value edge knowledge. Property-based/fuzz tests (proptest) cover the NLP parser and recurrence engine, mirroring Gör't's fast-check suite.
- **Seam 2 — `döne-service` (orchestration + persistence).** Integration tests against a temporary SQLite database exercising the service API: task CRUD and quick-add, complete → spawn next occurrence and uncomplete → remove spawn, delete → trash snapshot → restore/permanent-delete, views CRUD and rail ordering, export/import round-trip, settings/prefs, search, and structured error codes.
- **No UI test harness.** `döne-ui` is not unit-tested in v1 (no egui harness). The UI is verified by a manual side-by-side matrix: Gör't's 18 Playwright spec files (119 tests) become a checklist document, sections checked off as the corresponding milestone lands, with a full pass against the running reference app at M8.
- **Prior art.** Gör't's own suite is the template, mapped 1:1 — its pure-module unit suites become the `döne-core` fixtures; its store-layer integration suite (in-memory DB) becomes the `döne-service` integration tests; its Playwright e2e specs become the manual matrix.

## Out of Scope

- REST/HTTP API in v1 — the service layer stands in for it; MCP and Outlook integrations are explicit post-v1 branches, with transport choice (named pipe vs localhost HTTP) and not-running behaviour decided then.
- Cloud sync/sharing and attachments (also unimplemented in Gör't).
- macOS/Linux *release* targets — the codebase stays cross-platform from day one; Windows is the primary dev/test target.
- Auto-update. Installer arrives at M8; before that, v1 runs as a self-contained portable binary.
- Gör't data migration: we do not read Gör't's database files, and import accepts only Döne's own export format.
- egui UI test harness of any kind.
- Numeric performance budgets — efficiency is a steering directive achieved structurally (native process, no HTTP loopback); M8 records startup/RAM/CPU observations next to the Electron baseline for reference only.

## Further Notes

- The full agreed plan lives in `docs/plan.md`; governing decisions in ADRs 0001 (no HTTP server — service layer) and 0002 (integrations go through the service layer); vocabulary in `GLOSSARY.md`.
- The reference app stays read-only and gitignored in `.ref/gort` for the entire development, runnable side-by-side (`npm start`) whenever behaviour needs comparing.
- The divergence log is created at M1; every intentional behavioural difference from Gör't (found bugs/quirks fixed, plus the notification-while-closed improvement) is recorded there with rationale and a pinning test. Nothing is sacred, nothing is skipped.
- When implementation starts, this spec cuts into one issue per milestone under `.scratch/done/issues/` (`01-m0-scaffolding.md` … `08-m8-polish-installer.md`).
