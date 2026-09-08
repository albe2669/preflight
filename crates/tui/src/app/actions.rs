//! GraphQL mutation spawning. Each method clones the client and channel
//! sender, captures the current generation, and spawns a tokio task. When
//! `self.tx` is `None` (unit tests without a channel), every method no-ops.

use crate::app::{App, AppMsg, ToastKind};
impl App {
    pub(crate) fn spawn_refresh(&self) {
        let Some(tx) = &self.tx else { return };
        let date = self.logical_date.format("%Y-%m-%d").to_string();
        let client = self.client.clone();
        let tx = tx.clone();
        let r#gen = self.generation;
        tokio::spawn(async move {
            match client.fetch_all(&date).await {
                Ok(d) => {
                    let _ = tx.send(AppMsg::FetchAll { r#gen, data: d }).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_create_todo(&self, title: &str, plan_today: bool) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let title = title.to_string();
        tokio::spawn(async move {
            match client.create_todo(&title).await {
                Ok(todo) => {
                    if plan_today {
                        if let Err(e) = client.plan_today(todo.id).await {
                            let _ = tx
                                .send(AppMsg::Toast(ToastKind::Error, format!("plan failed: {e}")))
                                .await;
                        }
                    }
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "todo created".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_update_todo(&self, id: i32, title: Option<&str>, desc: Option<&str>) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let title = title.map(|s| s.to_string());
        let desc = desc.map(|s| s.to_string());
        tokio::spawn(async move {
            match client
                .update_todo(id, title.as_deref(), desc.as_deref())
                .await
            {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "saved".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_set_status(&self, id: i32, status: &str, blocked: Option<&str>) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let status = status.to_string();
        let blocked = blocked.map(|s| s.to_string());
        tokio::spawn(async move {
            match client.set_status(id, &status, blocked.as_deref()).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "status set".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_plan_today(&self, todo_id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match client.plan_today(todo_id).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(
                            ToastKind::Success,
                            "planned for today".into(),
                        ))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_unplan(&self, todo_id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            if client.unplan_today(todo_id).await.is_ok() {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Success, "unplanned".into()))
                    .await;
                let _ = tx.send(AppMsg::Refresh).await;
            }
        });
    }

    pub(crate) fn spawn_cancel(&self, id: i32) {
        self.spawn_set_status(id, "cancelled", None);
    }

    pub(crate) fn spawn_reorder(&self, date: &str, ids: &[i32]) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let date = date.to_string();
        let ids = ids.to_vec();
        tokio::spawn(async move {
            if client.reorder(&date, &ids).await.is_ok() {
                let _ = tx.send(AppMsg::Refresh).await;
            }
        });
    }

    pub(crate) fn spawn_carry_over(&self, from: &str, to: &str) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let from = from.to_string();
        let to = to.to_string();
        tokio::spawn(async move {
            match client.carry_over(&from, &to).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "carried over".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_add_tag(&self, todo_id: i32, slug: &str) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let slug = slug.to_string();
        tokio::spawn(async move {
            match client.add_tag(todo_id, &slug).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "tag added".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_remove_tag(&self, todo_id: i32, slug: &str) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let slug = slug.to_string();
        tokio::spawn(async move {
            match client.remove_tag(todo_id, &slug).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "tag removed".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_link_pr(&self, todo_id: i32, pr_id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match client
                .link_pull_request(todo_id, pr_id, crate::gql::LinkRelation::References)
                .await
            {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "pr linked".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_unlink_pr(&self, todo_id: i32, pr_id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            if client.unlink_pull_request(todo_id, pr_id).await.is_ok() {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Success, "pr detached".into()))
                    .await;
                let _ = tx.send(AppMsg::Refresh).await;
            }
        });
    }

    pub(crate) fn spawn_link_linear(&self, todo_id: i32, issue_id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match client.link_linear_issue(todo_id, issue_id).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "issue linked".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_unlink_linear(&self, todo_id: i32, issue_id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            if client.unlink_linear_issue(todo_id, issue_id).await.is_ok() {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Success, "issue detached".into()))
                    .await;
                let _ = tx.send(AppMsg::Refresh).await;
            }
        });
    }

    pub(crate) fn spawn_todo_from_pr(&self, pr_id: i32, plan_today: bool) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match client.todo_from_pr(pr_id, plan_today).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(
                            ToastKind::Success,
                            "converted to todo".into(),
                        ))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_dismiss_pr(&self, id: i32) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            if client.dismiss_pr(id).await.is_ok() {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Success, "dismissed".into()))
                    .await;
                let _ = tx.send(AppMsg::Refresh).await;
            }
        });
    }

    pub(crate) fn spawn_todo_from_linear(&self, issue_id: i32, plan_today: bool) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match client.todo_from_linear(issue_id, plan_today).await {
                Ok(_) => {
                    let _ = tx
                        .send(AppMsg::Toast(
                            ToastKind::Success,
                            "converted to todo".into(),
                        ))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_sync_github(&self) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.try_send(AppMsg::SyncStarted {
            source: "github".into(),
        });
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = client.sync_github().await;
            let success = result.is_ok();
            let _ = tx
                .send(AppMsg::SyncDone {
                    source: "github".into(),
                    success,
                })
                .await;
            if let Err(e) = result {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                    .await;
            }
        });
    }

    pub(crate) fn spawn_sync_linear(&self) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.try_send(AppMsg::SyncStarted {
            source: "linear".into(),
        });
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = client.sync_linear().await;
            let success = result.is_ok();
            let _ = tx
                .send(AppMsg::SyncDone {
                    source: "linear".into(),
                    success,
                })
                .await;
            if let Err(e) = result {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                    .await;
            }
        });
    }

    pub(crate) fn spawn_sync_source(&self, source: &str) {
        if source == "linear" {
            self.spawn_sync_linear();
        } else {
            self.spawn_sync_github();
        }
    }

    pub(crate) fn spawn_sync_all(&self) {
        let Some(tx) = &self.tx else { return };
        let _ = tx.try_send(AppMsg::SyncStarted {
            source: "linear".into(),
        });
        let _ = tx.try_send(AppMsg::SyncStarted {
            source: "github".into(),
        });
        let client = self.client.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let lin = client.sync_linear().await;
            let _ = tx
                .send(AppMsg::SyncDone {
                    source: "linear".into(),
                    success: lin.is_ok(),
                })
                .await;
            if let Err(e) = lin {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                    .await;
            }
            let gh = client.sync_github().await;
            let _ = tx
                .send(AppMsg::SyncDone {
                    source: "github".into(),
                    success: gh.is_ok(),
                })
                .await;
            if let Err(e) = gh {
                let _ = tx
                    .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                    .await;
            }
        });
    }

    pub(crate) fn spawn_fetch_detail(&mut self, id: i32) {
        self.detail_loaded_id = Some(id);
        self.detail = None;
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let r#gen = self.generation;
        tokio::spawn(async move {
            let result = tokio::join!(
                client.todo_events(id),
                client.todo_pull_requests(id),
                client.todo_linear_issues(id),
            );
            match result {
                (Ok(events), Ok(prs), Ok(linears)) => {
                    let _ = tx
                        .send(AppMsg::DetailData {
                            r#gen,
                            id,
                            events,
                            prs,
                            linears,
                        })
                        .await;
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_fetch_review(&self) {
        let Some(tx) = &self.tx else { return };
        let date = self.review_date.format("%Y-%m-%d").to_string();
        let client = self.client.clone();
        let tx = tx.clone();
        let r#gen = self.generation;
        tokio::spawn(async move {
            match client.daily_review(&date).await {
                Ok(r) => {
                    let _ = tx.send(AppMsg::DailyReview { r#gen, review: r }).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }

    pub(crate) fn spawn_sidebar_add_create(
        &self,
        title: &str,
        desc: &str,
        pending_prs: &[i32],
        plan_after: bool,
    ) {
        let Some(tx) = &self.tx else { return };
        let client = self.client.clone();
        let tx = tx.clone();
        let title = title.to_string();
        let desc = desc.to_string();
        let pending = pending_prs.to_vec();
        tokio::spawn(async move {
            match client.create_todo(&title).await {
                Ok(todo) => {
                    if !desc.is_empty() {
                        let _ = client.update_todo(todo.id, None, Some(&desc)).await;
                    }
                    for pr_id in &pending {
                        let _ = client
                            .link_pull_request(
                                todo.id,
                                *pr_id,
                                crate::gql::LinkRelation::References,
                            )
                            .await;
                    }
                    if plan_after {
                        let _ = client.plan_today(todo.id).await;
                    }
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Success, "todo created".into()))
                        .await;
                    let _ = tx.send(AppMsg::Refresh).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppMsg::Toast(ToastKind::Error, e.to_string()))
                        .await;
                }
            }
        });
    }
}
