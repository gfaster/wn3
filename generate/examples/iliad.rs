use generate::{
    chapter::{Chapter, ChapterBuilder},
    epub::EpubBuilder,
};

fn content() -> Vec<Chapter<'static>> {
    let mut b = ChapterBuilder::new();
    b.preserve_line_feeds(true).title_set("LIB. I.");

    for stanza in CONTENT.split("\n\n") {
        b.add_text(stanza).paragraph_finish();
    }

    b.finish().unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut b = EpubBuilder::new();
    b.add_identifier(generate::epub::IdentifierType::Adhoc, "The Iliad")
        .set_title("Iliad")
        .add_author("Homer")
        .add_translator("Thomas Hobbes")
        .add_editor("Sir William Molesworth")
        .extend_chapters(content());

    let out = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(true)
        .open("iliad.epub")?;
    b.finish(out)?;
    Ok(())
}

const CONTENT: &str = r#"The discontent and secession of Achilles.
O goddess sing what woe the discontent
Of Thetis’ son brought to the Greeks; what souls
Of heroes down to Erebus it sent,
Leaving their bodies unto dogs and fowls;
Whilst the two princes of the army strove,

King Agamemnon and Achilles stout.
That so it should be was the will of Jove,
But who was he that made them first fall out?
Apollo; who incensed by the wrong
To his priest Chryses by Atrides done,

Sent a great pestilence the Greeks among;
Apace they died, and remedy was none.
For Chryses came unto the Argive fleet,
With treasure great his daughter to redeem;
And having in his hands the ensigns meet,"#;
