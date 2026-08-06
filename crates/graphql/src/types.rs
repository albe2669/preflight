//! GraphQL output types that don't map directly to entity Models.

use seaography::CustomOutputType;

// The ActiveEnum -> Seaography bridges live in their respective domain crates
// to satisfy the orphan rule.  `register_enumeration` here builds the
// dynamic GraphQL Enum objects.

/// GraphQL representation of a daily review.
///
/// `todo_domain::DailyReview` derives `Serialize`/`Deserialize` but not
/// `CustomOutputType`. This struct mirrors it and carries the derive.
///
/// Entity `Model`s implement `CustomOutputType` via the `GqlModelType`
/// blanket impl (active because our seaography dep does NOT enable
/// `strict-custom-types`). No `impl_custom_output_type_for_entity!` bridge
/// is needed.
#[derive(Clone, CustomOutputType)]
pub struct DailyReview {
    pub date: chrono::NaiveDate,
    pub planned: Vec<todo_domain::entity::todo::Model>,
    pub touched: Vec<todo_domain::entity::todo::Model>,
    pub completed: Vec<todo_domain::entity::todo::Model>,
    pub carriedOver: Vec<todo_domain::entity::todo::Model>,
}

impl From<todo_domain::DailyReview> for DailyReview {
    fn from(d: todo_domain::DailyReview) -> Self {
        Self {
            date: d.date,
            planned: d.planned,
            touched: d.touched,
            completed: d.completed,
            carriedOver: d.carried_over,
        }
    }
}
