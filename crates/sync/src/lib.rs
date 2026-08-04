//! One-way remote pullers.
//!
//! Reads from GitHub / Linear, upserts local inbox tables, and persists
//! cursor state in `sync_state`. Tokens may be empty — the server boots
//! without network by taking a no-op path.

pub mod cursor;
pub mod github;
pub mod linear;
