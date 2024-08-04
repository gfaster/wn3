const HTML: &str = "<!DOCTYPE html><html><head></head><div class=\"entry-content\"><p>Sponsored Chapter!</p><hr><p>start of text</p></div></html>";

#[test]
fn generated() {
    let html = scraper::Html::parse_document(HTML);
    #[allow(unused)]
    let (ch, next) = wn3::common::Rules::new_il()
        .parse(&html)
        .expect("failed to parse");
    let t = ch.md().to_string();
    println!("==== begin output ====\n{t}\n====  end output  ====");
    assert!(
        !t.contains("Sponsored Chapter!"),
        "output contains \"Sponsored Chapter!\" incorrectly"
    );
    assert!(!t.contains("---"), "output contains \"---\" incorrectly");
    assert!(
        t.contains("start of text"),
        "output incorrectly contains \"start of text\""
    );
}
