use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

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

// Bridge PullRequestState to Seaography custom input/output types so it can appear
// as a GraphQL enum argument and return type in hand-written mutations.
// This impl must live in the defining crate (orphan rule).

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
