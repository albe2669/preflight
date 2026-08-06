pub mod enums;
pub mod pull_request;

seaography::register_entity_modules!([pull_request]);

seaography::impl_custom_output_type_for_entity!(pull_request::Model);
