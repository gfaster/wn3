use generate::{chapter::ChapterBuilder, epub::EpubBuilder};

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation
ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in
voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non
proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut b = EpubBuilder::new();
    b.add_identifier(generate::epub::IdentifierType::Adhoc, "example")
        .set_title("A book with many chapters")
        .add_author("John Epub")
        .set_chunk_size(10_000);

    for i in 0..5_000 {
        let mut ch = ChapterBuilder::new();
        ch.preserve_line_feeds(true)
            .title_set(format!("Chapter {i}"));

        ch.add_text(LOREM);

        b.add_chapter(ch.finish().unwrap().swap_remove(0));
    }

    let out = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(true)
        .open("many_small.epub")?;
    b.finish(out)?;
    Ok(())
}
