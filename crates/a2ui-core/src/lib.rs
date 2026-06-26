//! a2ui-core — A2UI Protocol v1.0 核心类型定义

mod error;
pub use error::{A2uiError, Result};
pub mod state;
pub mod component;
pub mod datamodel;
pub mod message;
pub use message::{ClientEnvelope, ServerEnvelope};
pub use component::ComponentId;
pub use component::Catalog;
pub use datamodel::DataModel;
pub mod prelude;

#[cfg(feature = "embed-assets")]
pub fn load_basic_catalog() -> Result<Catalog> {
    let json = include_str!("assets/catalogs/basic/catalog.json");
    let catalog: Catalog = serde_json::from_str(json)?;
    catalog.validate()?;
    Ok(catalog)
}
