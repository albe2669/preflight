//! Infrastructure shared by the remote sync providers (github, linear).
//!
//! Unlike the domain crates, `remote-sync` carries no domain models or HTTP
//! client bindings: it is the plumbing both providers duplicate — cursor
//! storage, pagination, response-error mapping — parameterized over the
//! provider's own client type. Domain crates depend on this crate; it never
//! depends on them.

pub mod cursor;
pub mod error;
pub mod page;
pub mod sync;
