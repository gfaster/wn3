const HTML: &str = "<!DOCTYPE html><html><head></head><main id=\"main\"><p>Sponsored Chapter!</p><p>start of text</p></main></html>";

#[test]
fn t() {
    let html = scraper::Html::parse_document(HTML);
    #[allow(unused)]
    let (ch, next) = wn3::common::Rules::new()
        .parse(&html)
        .expect("failed to parse");
    let t = format!("{ch:#}");
    println!("==== begin output ====\n{t}\n====  end output  ====");
    assert!(
        !t.contains("Sponsored Chapter!"),
        "output contains \"Sponsored Chapter!\" incorrectly"
    );
    assert!(
        t.contains("start of text"),
        "output incorrectly contains \"start of text\""
    );
}
