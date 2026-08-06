pub mod enums;
pub mod todo_linear_issue;
pub mod todo_pull_request;

seaography::register_entity_modules!([todo_pull_request, todo_linear_issue]);

seaography::impl_custom_output_type_for_entity!(todo_pull_request::Model);
seaography::impl_custom_output_type_for_entity!(todo_linear_issue::Model);
