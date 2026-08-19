// Smoke-test the Raycast GraphQL client queries against a live backend.
// Usage: node raycast/scripts/smoke.mjs [baseurl]
async function gql(q, v, base) {
  const r = await fetch(base, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ query: q, variables: v }),
  });
  return r.json();
}

const base = process.argv[2] || "http://127.0.0.1:8001/";

function out(k, v) {
  console.log(`${k}: ${JSON.stringify(v).slice(0, 600)}`);
}

try {
  out("syncState", await gql("{ syncState { nodes { source lastStatus lastSyncedAt lastError cursor } } }", null, base));
  out("plan", await gql(
    "query($f: TodoDayPlanFilterInput, $o: TodoDayPlanOrderInput) { todoDayPlan(filters: $f, orderBy: $o) { nodes { id planDate todoId position carriedOver addedAt removedAt todo { id title status } } } }",
    { f: { planDate: { eq: "2026-08-20" } }, o: { position: "ASC" } }, base));
  out("todo", await gql("{ todo { nodes { id title status blockedReason description } } }", null, base));
  out("pullRequest", await gql("{ pullRequest { nodes { id owner repo number title state reviewRequested authoredByMe dismissedAt } } }", null, base));
  out("linearIssue", await gql("{ linearIssue { nodes { id identifier title stateName stateType assignedToMe dismissedAt } } }", null, base));
  out("dailyReview", await gql("query($d: String!) { dailyReview(date: $d) { date planned { id title status } touched { id } completed { id } carriedOver { id } } }", { d: "2026-08-20" }, base));

  // Mutation sanity: create a throwaway todo, set status, add tag, plan it.
  const created = await gql("mutation($t: String!) { createTodo(title: $t) { id title status } }", { t: "__raycast_smoke__" }, base);
  out("createTodo", created);
  const id = created.data?.createTodo?.id;
  if (id != null) {
    out("setStatus", await gql("mutation($id: Int!, $s: TodoStatusEnum!) { setTodoStatus(id: $id, status: $s) { id status } }", { id, s: "done" }, base));
    out("addTag", await gql("mutation($id: Int!, $s: String!) { addTag(todoId: $id, slug: $s) { id title } }", { id, s: "smoke" }, base));
    out("planForToday", await gql("mutation($id: Int!) { planForToday(todoId: $id) { id planDate todoId position } }", { id }, base));
    out("unplan", await gql("mutation($id: Int!) { unplanForToday(todoId: $id) }", { id }, base));
    out("removeTag", await gql("mutation($id: Int!, $s: String!) { removeTag(todoId: $id, slug: $s) { id title } }", { id, s: "smoke" }, base));
    out("todoTags", await gql("query($f: TodoTagFilterInput) { todoTag(filters: $f) { nodes { tag { id slug name color } } } }", { f: { todoId: { eq: id } } }, base));
    out("todoEvents", await gql("query($f: TodoEventFilterInput, $o: TodoEventOrderInput) { todoEvent(filters: $f, orderBy: $o) { nodes { id kind actor occurredAt logicalDate } } }", { f: { todoId: { eq: id } }, o: { occurredAt: "ASC" } }, base));
  }
  console.log("\nSMOKE OK");
} catch (e) {
  console.log("SMOKE ERR:", e.message);
  process.exit(1);
}
