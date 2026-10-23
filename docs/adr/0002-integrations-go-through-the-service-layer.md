# External integrations (MCP, Outlook API) are clients of the live app, never of the database

Döne will later expose programmatic integrations: an MCP server for AI agents and a local API for things like Outlook macros creating tasks. These run in separate processes and need access to app functionality.

We decided: **all integrations go through `döne-service` as clients of the *running* Döne process** (over a local transport, chosen later). No integration ever opens the database directly. Consequently, `döne-service`'s API is designed from day one as a foreign-process interface: explicit methods, plain data in and out, no UI types across the boundary, transport-agnostic.

Why not let integrations write to SQLite directly (WAL mode would prevent corruption)? The GUI process owns in-memory state — current view, undo stack, notification dedupe. A task inserted behind its back never appears in the open window, and the GUI's next write from stale state can clobber it. Routing external writes through the service layer means they take the exact same code path as UI actions (validation, recurrence spawn, trash, activity log), and the GUI can refresh to show them.

**Considered options:** direct DB access from integrations (rejected: stale-state clobbering is fatal for UX even though WAL prevents corruption); per-integration custom access layers (rejected: each would re-implement or skip domain rules). MCP server process model, transport choice, and not-running behaviour are deferred post-v1 decisions.
