# Glossary

Words we use in this project, and the words we avoid.

## Terms

| Term | Meaning |
|---|---|
| **Döne** | This app: the Rust + egui reimplementation of Gör't. A single native binary; local-first To Do app. |
| **Gör't** | The reference app in `.ref/gort` (Node.js + vanilla JS + Electron). Read-only source of truth for behaviour and data model. Never modified. |
| **System list** | A list the app creates itself: My Day, Shortlisted, Important, Planned, All (page title "All tasks"), Completed. The user cannot add or remove them. |
| **My Day** | The computed agenda for today: open tasks in user-ordered groups split by a section break into "Do first" and "Continue with", with completed-today pinned at the bottom. Tasks expire at midnight. |
| **View** | A saved filter the user makes: name, icon, tags, match any/all, sort levels, include-completed toggle. Shown in the rail. |
| **Tag** | A `#word` on a task. Tags and views replace user-made lists. |
| **Focus date** | The single date field a task was picked for; a task counts as shortlisted while its focus date is today. |
| **Section break** | The marker in My Day's group order that splits it into "Do first" and "Continue with". User-movable. |
| **Parity fixture** | A test case ported from Gör't's suite, reviewed at port time: kept as-is if it encodes intended behaviour, replaced by a divergence test if it encoded a quirk or bug. |
| **Divergence** | An intentional behavioural difference from Gör't (a found bug/quirk, fixed). Recorded in `docs/divergences.md` with rationale and a test pinning the new behaviour. |

## Words to avoid

| Avoid | Why | Use instead |
|---|---|---|
| "the port", "the rewrite" | Implies a mechanical translation; Döne is a clean-room reimplementation with its own architecture. | **Döne** (this app), **Gör't** (the reference) |
| **list** for a user-made collection | User-made lists do not exist in this domain. | **view** (saved filter) or **tag** |
