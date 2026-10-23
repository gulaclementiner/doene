# No HTTP server: the REST surface becomes an in-process service layer

Gör't exposes a 20-endpoint localhost REST API because its UI is a browser talking to a Node server. Döne is a native egui app whose UI runs in the same process as the data, so an HTTP layer would be pure overhead (server thread, framework dependency, port management) with no user-visible benefit.

We decided: **no HTTP server in v1.** The reference API's operations and structured error codes (`{ code, params }`) become methods on an in-process service layer that the UI calls directly. The service layer is the designated seam: an optional API crate can expose it over HTTP later without touching core, storage, or UI.

**Considered options:** ship the REST API anyway for parity (rejected: zero user-facing value in a native app, real cost); let the UI call the store directly (rejected: couples UI to persistence and leaves no seam for a future API).
