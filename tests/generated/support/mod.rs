
pub fn match_test_v1(file: &str, pat: &str) {
    let rule = wn3::common::Rules::new_il();
    let path = format!("tests/generated/{file}.input.html");
    let document = std::fs::read_to_string(path).unwrap();
    let html = scraper::Html::parse_document(&document);
    let chapter = rule.parse(&html).expect("failed to parse chapter");
    let rendered = chapter.0.to_string();
    let regex = regex_lite::Regex::new(pat).expect("invalid pattern");
    assert!(!regex.is_match(&rendered), "chapter matched regex /{regex}/\n\n{rendered}");
}
