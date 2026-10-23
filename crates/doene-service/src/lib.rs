//! Döne service layer: application operations orchestrating core rules and storage.
//!
//! The designated integration seam (ADR-0001, ADR-0002): explicit methods, plain
//! data in and out, no UI types across the boundary, transport-agnostic.
//! Operations land in M2; this crate is scaffolding until then.
