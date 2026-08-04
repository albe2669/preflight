#![allow(non_snake_case)]
pub mod entities;
pub mod mutation;
pub mod query;
pub mod query_root;
pub mod types;

pub use query_root::{schema, schema_builder};
