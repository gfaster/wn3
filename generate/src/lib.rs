// use scraper::{Html, Selector};

pub mod chapter;
pub use chapter::{Chapter, ChapterBuilder};
mod html_writer;
pub mod image;
mod jacket;
pub mod lang;
mod util;

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
