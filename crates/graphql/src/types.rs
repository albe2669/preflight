//! GraphQL output types that don't map directly to entity Models.

use seaography::CustomOutputType;

// The 5 ActiveEnum → Seaography bridges now live in the `entity` crate
// (crates/entity/src/sea_orm_active_enums.rs) to satisfy the orphan rule.
// `register_enumeration` here builds the dynamic GraphQL Enum objects.

/// GraphQL representation of a daily review.
///
/// `preflight_core::DailyReview` derives `Serialize`/`Deserialize` but not
/// `CustomOutputType`. This struct mirrors it and carries the derive.
///
/// Entity `Model`s implement `CustomOutputType` via the `GqlModelType`
/// blanket impl (active because our seaography dep does NOT enable
/// `strict-custom-types`). No `impl_custom_output_type_for_entity!` bridge
/// is needed.
#[derive(Clone, CustomOutputType)]
pub struct DailyReview {
    pub date: chrono::NaiveDate,
    pub planned: Vec<entity::todo::Model>,
    pub touched: Vec<entity::todo::Model>,
    pub completed: Vec<entity::todo::Model>,
    pub carriedOver: Vec<entity::todo::Model>,
}

impl From<preflight_core::DailyReview> for DailyReview {
    fn from(d: preflight_core::DailyReview) -> Self {
        Self {
            date: d.date,
            planned: d.planned,
            touched: d.touched,
            completed: d.completed,
            carriedOver: d.carried_over,
        }
    }
}
