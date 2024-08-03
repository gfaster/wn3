const HTML: &str = "<!DOCTYPE html><html><head></head><div class=\"entry-content\"><ul><li>Chapter 95: Part 2</li></ul><p>start of more</p></div></html>";

#[test]
fn generated() {
    let html = scraper::Html::parse_document(HTML);
    #[allow(unused)]
    let (ch, next) = wn3::common::Rules::new_il()
        .parse(&html)
        .expect("failed to parse");
    let t = format!("{ch:#}");
    println!("==== begin output ====\n{t}\n====  end output  ====");
    assert!(t.contains("# Chapter 95: Part 2"));
}
