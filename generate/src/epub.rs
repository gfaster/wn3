pub(crate) mod package;
mod xml;
mod book;

pub use book::EpubBuilder;
pub use package::{IdentifierType, ManifestItem, ManifestProperties};
