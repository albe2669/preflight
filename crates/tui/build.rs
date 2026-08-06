//! Cynic codegen: registers the shared `api/schema.graphql` so the
//! `#[cynic::schema]` module in `gql.rs` produces type-safe query structs.
//!
//! To update the schema, run `cargo run -p tui --bin introspect-schema`
//! (or `devenv task run gen:schema`), which writes `api/schema.graphql`.

fn main() {
    cynic_codegen::register_schema("preflight")
        .from_sdl_file("../../api/schema.graphql")
        .unwrap()
        .as_default()
        .unwrap();
}
