//! Custom queries registered with the Seaography builder.

pub struct Queries;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl Queries {
    pub async fn dailyReview(
        ctx: &async_graphql::Context<'_>,
        date: chrono::NaiveDate,
    ) -> async_graphql::Result<crate::types::DailyReview> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let review = preflight_core::ReviewService::new(&db, &clock)
            .daily(date)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(review.into())
    }
}
