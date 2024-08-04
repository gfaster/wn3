use std::fmt::Display;

use crate::Chapter;
pub(super) mod md;
pub(super) mod xml;

pub(super) trait SerChapter: Sized + Copy {
    fn disp<'a>(self, el: &'a Chapter) -> impl Display + 'a;
}
