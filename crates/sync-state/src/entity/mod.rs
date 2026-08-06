pub mod sync_state;

seaography::register_entity_modules!([sync_state]);

seaography::impl_custom_output_type_for_entity!(sync_state::Model);
