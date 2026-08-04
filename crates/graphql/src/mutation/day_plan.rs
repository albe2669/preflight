//! Day plan mutations: plan, unplan, reorder, carry over.

use async_graphql;

pub struct DayPlanMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl DayPlanMutations {
    pub async fn planForToday(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
    ) -> async_graphql::Result<entity::todo_day_plan::Model> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let plan = preflight_core::DayPlanService::new(&db, &clock)
            .plan_today(todoId)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(plan)
    }

    pub async fn unplanForToday(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
    ) -> async_graphql::Result<bool> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        preflight_core::DayPlanService::new(&db, &clock)
            .unplan_today(todoId)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        // core returns Ok(()) whether or not a row existed; return true on success.
        Ok(true)
    }

    pub async fn reorderDayPlan(
        ctx: &async_graphql::Context<'_>,
        date: chrono::NaiveDate,
        todoIds: Vec<i64>,
    ) -> async_graphql::Result<Vec<entity::todo_day_plan::Model>> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let plans = preflight_core::DayPlanService::new(&db, &clock)
            .reorder(date, &todoIds)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(plans)
    }

    pub async fn carryOverUnfinished(
        ctx: &async_graphql::Context<'_>,
        from: chrono::NaiveDate,
        to: chrono::NaiveDate,
    ) -> async_graphql::Result<Vec<entity::todo_day_plan::Model>> {
        let db = ctx.data::<sea_orm::DatabaseConnection>().unwrap().clone();
        let clock = ctx.data::<preflight_core::Clock>().unwrap().clone();
        let plans = preflight_core::DayPlanService::new(&db, &clock)
            .carry_over(from, to)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(plans)
    }
}
