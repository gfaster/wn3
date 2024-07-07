use crate::chapter::Chapter;

pub struct EpubBuilder<'a> {
    chapters: Vec<Chapter<'a>>
}
