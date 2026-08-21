#![cfg(feature = "e2e")]

use e2e::{gql_post, spawn};

/// End-to-end todo lifecycle: create, fetch by id, plan for today, set
/// status, add a tag, and confirm the daily review lists it as planned and
/// touched.
#[tokio::test(flavor = "current_thread")]
async fn todo_create_fetch_plan_status_tag_flow() {
    let server = spawn().await.expect("failed to spawn server");
    let url = server.graphql_url();

    let create = gql_post(
        &url,
        r#"
        mutation { createTodo(title: "Write e2e tests", description: "first pass") {
            id title description status
        } }
    "#,
    )
    .await
    .unwrap();
    let todo = &create["data"]["createTodo"];
    let id = todo["id"].as_i64().unwrap();
    assert_eq!(todo["title"], "Write e2e tests");
    assert_eq!(todo["status"], "todo");

    let fetch = gql_post(
        &url,
        &format!(
            r#"
        query {{ todo(filters: {{ id: {{ eq: {} }} }}) {{ nodes {{ id title status }} }} }}
    "#,
            id
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        fetch["data"]["todo"]["nodes"][0]["title"],
        "Write e2e tests"
    );

    let plan = gql_post(
        &url,
        &format!(
            r#"
        mutation {{ planForToday(todoId: {}) {{ id position }} }}
    "#,
            id
        ),
    )
    .await
    .unwrap();
    assert_eq!(plan["data"]["planForToday"]["position"], 0);

    let status = gql_post(
        &url,
        &format!(
            r#"
        mutation {{ setTodoStatus(id: {}, status: started) {{ id status }} }}
    "#,
            id
        ),
    )
    .await
    .unwrap();
    assert_eq!(status["data"]["setTodoStatus"]["status"], "started");

    let tag = gql_post(
        &url,
        &format!(
            r#"
        mutation {{ addTag(todoId: {}, slug: "urgent") {{ id }} }}
    "#,
            id
        ),
    )
    .await
    .unwrap();
    assert!(tag.get("errors").is_none());

    let tag_q = gql_post(
        &url,
        r#"{ tag(filters: { slug: { eq: "urgent" } }) { nodes { slug } } }"#,
    )
    .await
    .unwrap();
    assert_eq!(tag_q["data"]["tag"]["nodes"][0]["slug"], "urgent");

    let clock_q = gql_post(&url, "query { clock { logicalDate } }")
        .await
        .unwrap();
    let logical_date = clock_q["data"]["clock"]["logicalDate"]
        .as_str()
        .unwrap()
        .to_string();
    let review = gql_post(
        &url,
        &format!(
            r#"
        query {{ dailyReview(date: "{}") {{ planned {{ id }} touched {{ id }} }} }}
    "#,
            logical_date
        ),
    )
    .await
    .unwrap();
    let planned_ids: Vec<i64> = review["data"]["dailyReview"]["planned"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_i64().unwrap())
        .collect();
    let touched_ids: Vec<i64> = review["data"]["dailyReview"]["touched"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_i64().unwrap())
        .collect();
    assert!(planned_ids.contains(&id));
    assert!(touched_ids.contains(&id));

    assert!(create.get("errors").is_none());
    assert!(fetch.get("errors").is_none());
    assert!(plan.get("errors").is_none());
    assert!(status.get("errors").is_none());
    assert!(tag.get("errors").is_none());
    assert!(tag_q.get("errors").is_none());
    assert!(review.get("errors").is_none());

    server.shutdown().await;
}

/// An empty-token `syncGithub` is a no-op that records status `"never"`.
#[tokio::test(flavor = "current_thread")]
async fn sync_with_empty_token_is_noop() {
    let server = spawn().await.expect("failed to spawn server");
    let url = server.graphql_url();

    let sync = gql_post(
        &url,
        "mutation { syncGithub { source lastStatus lastError } }",
    )
    .await
    .unwrap();
    assert_eq!(sync["data"]["syncGithub"]["source"], "github");
    assert_eq!(sync["data"]["syncGithub"]["lastStatus"], "never");
    assert!(sync.get("errors").is_none());

    server.shutdown().await;
}
