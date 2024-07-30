// use scraper::{Html, Selector};

pub mod chapter;
pub use chapter::{ChapterBuilder, Chapter};
mod html_writer;
mod util;
pub mod image;

pub mod epub;
pub use epub::EpubBuilder;
pub mod epubcheck;

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//     }
// }
