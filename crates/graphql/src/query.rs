//! Custom queries registered with the Seaography builder.

use std::sync::Arc;

pub struct Queries;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl Queries {
    pub async fn dailyReview(
        ctx: &async_graphql::Context<'_>,
        date: chrono::NaiveDate,
    ) -> async_graphql::Result<crate::types::DailyReview> {
        let review_svc = ctx
            .data::<Arc<dyn todo_domain::ReviewService>>()
            .unwrap()
            .clone();
        let review = review_svc
            .daily(date)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(review.into())
    }

    pub async fn clock(
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<crate::types::Clock> {
        let clock = ctx.data::<todo_domain::Clock>().unwrap().clone();
        Ok(crate::types::Clock {
            logicalDate: clock.now_logical(),
            timezone: clock.tz.to_string(),
            dayStartHour: clock.day_start_hour,
        })
    }
}
