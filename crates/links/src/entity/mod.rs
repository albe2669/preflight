pub mod enums;
pub mod todo_linear_issue;
pub mod todo_pull_request;

seaography::register_entity_modules!([todo_pull_request, todo_linear_issue]);
