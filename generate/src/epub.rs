mod book;
pub(crate) mod package;
mod xml;

pub use book::EpubBuilder;
pub use package::{IdentifierType, ManifestItem, ManifestProperties};
