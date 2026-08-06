pub mod entity;
pub mod error;
pub mod link_service;

pub use error::{LinkError, Result};
pub use link_service::LinkService;
pub use link_service::new;
