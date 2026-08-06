use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

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

// ---------- LinkRelation Seaography impls ----------

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
