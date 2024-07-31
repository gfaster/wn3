use std::sync::Arc;

use fetch::FetchContext;

fn main() {
    let db = rusqlite::Connection::open("example_simple.db");
    let client = ureq::AgentBuilder::new()
        .user_agent("Mozilla/5.0 (ratelimited fetch example)")
        .build();
    let cx = Arc::new(FetchContext::new(db.unwrap(), client).unwrap());

    let urls = [
        "http://httpbin.org/image/jpeg",
        "http://httpbin.org/image/png",
        "http://httpbin.org/image/svg",
        "https://example.com",
        "https://github.com",
        "https://docs.rs",
        "https://crates.io",
        "https://rust-lang.org",
        "https://gnu.org",
        "https://mozilla.org",
        "https://example.com",
    ];

    for url in urls {
        let cx = Arc::clone(&cx);
        cx.fetch(&url::Url::parse(url).unwrap()).unwrap();
        // tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}
