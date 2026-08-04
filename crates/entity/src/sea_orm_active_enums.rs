//! Hand-maintained ActiveEnums.
//!
//! SQLite stores every one of these as `TEXT`; SeaORM validates the value on
//! read, and Seaography surfaces each as a real GraphQL enum (registered via
//! `register_active_enums!`). The entity generator wrote these columns as
//! `String`; they are typed here so the database `CHECK` constraints are
//! reinforced by a Rust type, and so a client can never write an unknown
//! value through GraphQL.
//!
//! `EventKind` and `PullRequestState` intentionally have **no** SQL `CHECK`:
//! both mirror domains that grow without asking. The enum is the guard on
//! read; unknown values surface as a SeaORM error. Add a variant when you
//! adopt a new value, never a SQL migration.
//!
//! Each variant's `#[serde(rename = ...)]` matches its `#[sea_orm(string_value
//! = ...)]` so JSON round-trips use the same token the database stores.
//!
//! Hand-written `CustomInputType`/`CustomOutputType` impls at the foot of
//! this file bridge each enum to Seaography so they appear as real GraphQL
//! enum args and return types. The `impl_custom_type_for_enum!` macro was
//! replaced because it delegated to `GqlScalarValueType`, declaring enum
//! args as String scalars — which broke argument parsing.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// The status of a todo. Closed, stable domain — the migration enforces it
/// with a `CHECK`, and this enum enforces it on the Rust side.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum TodoStatus {
    #[sea_orm(string_value = "todo")]
    #[serde(rename = "todo")]
    Todo,
    #[sea_orm(string_value = "started")]
    #[serde(rename = "started")]
    Started,
    #[sea_orm(string_value = "blocked")]
    #[serde(rename = "blocked")]
    Blocked,
    #[sea_orm(string_value = "done")]
    #[serde(rename = "done")]
    Done,
    #[sea_orm(string_value = "cancelled")]
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// What kind of thing happened to a todo. Appends to `todo_event`; no SQL
/// `CHECK` because this set is expected to grow.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum EventKind {
    #[sea_orm(string_value = "created")]
    #[serde(rename = "created")]
    Created,
    #[sea_orm(string_value = "title_changed")]
    #[serde(rename = "title_changed")]
    TitleChanged,
    #[sea_orm(string_value = "description_changed")]
    #[serde(rename = "description_changed")]
    DescriptionChanged,
    #[sea_orm(string_value = "status_changed")]
    #[serde(rename = "status_changed")]
    StatusChanged,
    #[sea_orm(string_value = "blocked")]
    #[serde(rename = "blocked")]
    Blocked,
    #[sea_orm(string_value = "unblocked")]
    #[serde(rename = "unblocked")]
    Unblocked,
    #[sea_orm(string_value = "tag_added")]
    #[serde(rename = "tag_added")]
    TagAdded,
    #[sea_orm(string_value = "tag_removed")]
    #[serde(rename = "tag_removed")]
    TagRemoved,
    #[sea_orm(string_value = "linked_pr")]
    #[serde(rename = "linked_pr")]
    LinkedPr,
    #[sea_orm(string_value = "linked_linear")]
    #[serde(rename = "linked_linear")]
    LinkedLinear,
    #[sea_orm(string_value = "planned")]
    #[serde(rename = "planned")]
    Planned,
    #[sea_orm(string_value = "unplanned")]
    #[serde(rename = "unplanned")]
    Unplanned,
    #[sea_orm(string_value = "carried_over")]
    #[serde(rename = "carried_over")]
    CarriedOver,
}

/// Who caused an event. Load-bearing: a background sync must not make an
/// untouched todo look "worked on". The migration enforces this with a
/// `CHECK`; this enum enforces it in Rust.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum EventActor {
    #[sea_orm(string_value = "user")]
    #[serde(rename = "user")]
    User,
    #[sea_orm(string_value = "sync")]
    #[serde(rename = "sync")]
    Sync,
    #[sea_orm(string_value = "system")]
    #[serde(rename = "system")]
    System,
}

/// Mirrors the subset of GitHub PR states we ingest. No `CHECK`: remote domain.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum PullRequestState {
    #[sea_orm(string_value = "open")]
    #[serde(rename = "open")]
    Open,
    #[sea_orm(string_value = "closed")]
    #[serde(rename = "closed")]
    Closed,
    #[sea_orm(string_value = "merged")]
    #[serde(rename = "merged")]
    Merged,
    #[sea_orm(string_value = "draft")]
    #[serde(rename = "draft")]
    Draft,
}

/// How a todo relates to a pull request. Closed, stable domain — the
/// migration enforces it with a `CHECK`.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
pub enum LinkRelation {
    #[sea_orm(string_value = "reviews")]
    #[serde(rename = "reviews")]
    Reviews,
    #[sea_orm(string_value = "implements")]
    #[serde(rename = "implements")]
    Implements,
    #[sea_orm(string_value = "references")]
    #[serde(rename = "references")]
    References,
}

// Bridge each enum to Seaography's custom input/output types so they can appear
// as GraphQL enum arguments and return types in hand-written mutations. These
// impls must live in the defining crate (orphan rule).
//
// The `impl_custom_type_for_enum!` macro delegates to `GqlScalarValueType`, which
// declares enum args as String scalars — breaking parse_value which expects enum
// names. Hand-written impls return the correct enum TypeRef (`{Enum}Enum`).

// ---------- TodoStatus ----------

impl seaography::CustomInputType for TodoStatus {
    fn gql_input_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("TodoStatusEnum")
    }
    fn parse_value(
        _ctx: &'static seaography::BuilderContext,
        value: Option<async_graphql::dynamic::ValueAccessor<'_>>,
    ) -> seaography::SeaResult<Self> {
        match value {
            None => Err(seaography::SeaographyError::AsyncGraphQLError(
                "Value expected".into(),
            )),
            Some(v) => {
                let s = v.enum_name()?.to_string();
                <Self as sea_orm::ActiveEnum>::try_from_value(&s)
                    .map_err(|e| seaography::SeaographyError::AsyncGraphQLError(e.into()))
            }
        }
    }
}

impl seaography::CustomOutputType for TodoStatus {
    fn gql_output_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("TodoStatusEnum")
    }
    fn gql_field_value(
        self,
        _ctx: &'static seaography::BuilderContext,
    ) -> Option<async_graphql::dynamic::FieldValue<'static>> {
        let s = self.to_value();
        Some(async_graphql::dynamic::FieldValue::value(
            async_graphql::Value::Enum(async_graphql::Name::new(s)),
        ))
    }
}

// ---------- EventKind ----------

impl seaography::CustomInputType for EventKind {
    fn gql_input_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("EventKindEnum")
    }
    fn parse_value(
        _ctx: &'static seaography::BuilderContext,
        value: Option<async_graphql::dynamic::ValueAccessor<'_>>,
    ) -> seaography::SeaResult<Self> {
        match value {
            None => Err(seaography::SeaographyError::AsyncGraphQLError(
                "Value expected".into(),
            )),
            Some(v) => {
                let s = v.enum_name()?.to_string();
                <Self as sea_orm::ActiveEnum>::try_from_value(&s)
                    .map_err(|e| seaography::SeaographyError::AsyncGraphQLError(e.into()))
            }
        }
    }
}

impl seaography::CustomOutputType for EventKind {
    fn gql_output_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("EventKindEnum")
    }
    fn gql_field_value(
        self,
        _ctx: &'static seaography::BuilderContext,
    ) -> Option<async_graphql::dynamic::FieldValue<'static>> {
        let s = self.to_value();
        Some(async_graphql::dynamic::FieldValue::value(
            async_graphql::Value::Enum(async_graphql::Name::new(s)),
        ))
    }
}

// ---------- EventActor ----------

impl seaography::CustomInputType for EventActor {
    fn gql_input_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("EventActorEnum")
    }
    fn parse_value(
        _ctx: &'static seaography::BuilderContext,
        value: Option<async_graphql::dynamic::ValueAccessor<'_>>,
    ) -> seaography::SeaResult<Self> {
        match value {
            None => Err(seaography::SeaographyError::AsyncGraphQLError(
                "Value expected".into(),
            )),
            Some(v) => {
                let s = v.enum_name()?.to_string();
                <Self as sea_orm::ActiveEnum>::try_from_value(&s)
                    .map_err(|e| seaography::SeaographyError::AsyncGraphQLError(e.into()))
            }
        }
    }
}

impl seaography::CustomOutputType for EventActor {
    fn gql_output_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("EventActorEnum")
    }
    fn gql_field_value(
        self,
        _ctx: &'static seaography::BuilderContext,
    ) -> Option<async_graphql::dynamic::FieldValue<'static>> {
        let s = self.to_value();
        Some(async_graphql::dynamic::FieldValue::value(
            async_graphql::Value::Enum(async_graphql::Name::new(s)),
        ))
    }
}

// ---------- PullRequestState ----------

impl seaography::CustomInputType for PullRequestState {
    fn gql_input_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("PullRequestStateEnum")
    }
    fn parse_value(
        _ctx: &'static seaography::BuilderContext,
        value: Option<async_graphql::dynamic::ValueAccessor<'_>>,
    ) -> seaography::SeaResult<Self> {
        match value {
            None => Err(seaography::SeaographyError::AsyncGraphQLError(
                "Value expected".into(),
            )),
            Some(v) => {
                let s = v.enum_name()?.to_string();
                <Self as sea_orm::ActiveEnum>::try_from_value(&s)
                    .map_err(|e| seaography::SeaographyError::AsyncGraphQLError(e.into()))
            }
        }
    }
}

impl seaography::CustomOutputType for PullRequestState {
    fn gql_output_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("PullRequestStateEnum")
    }
    fn gql_field_value(
        self,
        _ctx: &'static seaography::BuilderContext,
    ) -> Option<async_graphql::dynamic::FieldValue<'static>> {
        let s = self.to_value();
        Some(async_graphql::dynamic::FieldValue::value(
            async_graphql::Value::Enum(async_graphql::Name::new(s)),
        ))
    }
}

// ---------- LinkRelation ----------

impl seaography::CustomInputType for LinkRelation {
    fn gql_input_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("LinkRelationEnum")
    }
    fn parse_value(
        _ctx: &'static seaography::BuilderContext,
        value: Option<async_graphql::dynamic::ValueAccessor<'_>>,
    ) -> seaography::SeaResult<Self> {
        match value {
            None => Err(seaography::SeaographyError::AsyncGraphQLError(
                "Value expected".into(),
            )),
            Some(v) => {
                let s = v.enum_name()?.to_string();
                <Self as sea_orm::ActiveEnum>::try_from_value(&s)
                    .map_err(|e| seaography::SeaographyError::AsyncGraphQLError(e.into()))
            }
        }
    }
}

impl seaography::CustomOutputType for LinkRelation {
    fn gql_output_type_ref(
        _ctx: &'static seaography::BuilderContext,
    ) -> async_graphql::dynamic::TypeRef {
        async_graphql::dynamic::TypeRef::named_nn("LinkRelationEnum")
    }
    fn gql_field_value(
        self,
        _ctx: &'static seaography::BuilderContext,
    ) -> Option<async_graphql::dynamic::FieldValue<'static>> {
        let s = self.to_value();
        Some(async_graphql::dynamic::FieldValue::value(
            async_graphql::Value::Enum(async_graphql::Name::new(s)),
        ))
    }
}
