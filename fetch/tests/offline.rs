use std::path::PathBuf;

use fetch::{FetchContext, MediaType};
use url::Url;

#[test]
fn offline_allows_file() {
    let a = FetchContext::new_cfg(
        rusqlite::Connection::open_in_memory().unwrap(),
        ureq::agent(),
        true,
    )
    .unwrap();
    let mut path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    path.push("testfile0.html");
    assert!(path.is_absolute());
    let contents = "<!DOCTYPE html> <html> <head> </head> <body> </body> </html>";
    std::fs::write(&path, contents).unwrap();
    let url = Url::from_file_path(&path).unwrap();
    assert_eq!(url.scheme(), "file");
    let (ty, res) = a.fetch(&url).unwrap();
    assert_eq!(ty, MediaType::Html);
    assert_eq!(res, contents.as_bytes());
}
