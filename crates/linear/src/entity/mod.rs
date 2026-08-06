pub mod linear_issue;

seaography::register_entity_modules!([linear_issue]);

seaography::impl_custom_output_type_for_entity!(linear_issue::Model);
