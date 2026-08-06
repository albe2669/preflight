//! Day plan mutations: plan, unplan, reorder, carry over.

use async_graphql;
use std::sync::Arc;

pub struct DayPlanMutations;

#[seaography::CustomFields]
#[allow(non_snake_case)]
impl DayPlanMutations {
    pub async fn planForToday(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
    ) -> async_graphql::Result<todo_domain::entity::todo_day_plan::Model> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::DayPlanService>>()
            .unwrap()
            .clone();
        let plan = svc
            .plan_today(todoId)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(plan)
    }

    pub async fn unplanForToday(
        ctx: &async_graphql::Context<'_>,
        todoId: i64,
    ) -> async_graphql::Result<bool> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::DayPlanService>>()
            .unwrap()
            .clone();
        svc.unplan_today(todoId)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        // core returns Ok(()) whether or not a row existed; return true on success.
        Ok(true)
    }

    pub async fn reorderDayPlan(
        ctx: &async_graphql::Context<'_>,
        date: chrono::NaiveDate,
        todoIds: Vec<i64>,
    ) -> async_graphql::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::DayPlanService>>()
            .unwrap()
            .clone();
        let plans = svc
            .reorder(date, &todoIds)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(plans)
    }

    pub async fn carryOverUnfinished(
        ctx: &async_graphql::Context<'_>,
        from: chrono::NaiveDate,
        to: chrono::NaiveDate,
    ) -> async_graphql::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        let svc = ctx
            .data::<Arc<dyn todo_domain::DayPlanService>>()
            .unwrap()
            .clone();
        let plans = svc
            .carry_over(from, to)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(plans)
    }
}
