use crate::{
    config::Config,
    db::{books::Book, store::Store},
    epub::parser,
    state::AppState,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use std::io::Write;
use tower::ServiceExt;

fn fixture(path: &std::path::Path, version: &str, chapter: &str, notes: &str) {
    fixture_with_titles(
        path,
        version,
        chapter,
        notes,
        &[Some("Actual chapter title")],
    );
}

fn fixture_with_titles(
    path: &std::path::Path,
    version: &str,
    chapter: &str,
    notes: &str,
    titles: &[Option<&str>],
) {
    let file = std::fs::File::create(path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut opf = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="{version}" unique-identifier="uid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">test</dc:identifier><dc:title>Fixture Book</dc:title><dc:creator>Test Author</dc:creator><dc:language>en</dc:language><meta property="dcterms:modified">2026-10-22T00:00:00Z</meta></metadata><manifest><item id="chapter" href="chapters/one.xhtml" media-type="application/xhtml+xml"/><item id="notes" href="notes.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="ncx"><itemref idref="chapter"/></spine></package>"#
    );
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut navigation = String::new();
    let mut points = String::new();
    for (index, title) in titles.iter().enumerate() {
        let href = if index == 0 {
            "chapters/one.xhtml".to_string()
        } else {
            manifest.push_str(&format!(r#"<item id="part{index}" href="chapters/part{index}.xhtml" media-type="application/xhtml+xml"/>"#));
            spine.push_str(&format!(r#"<itemref idref="part{index}"/>"#));
            format!("chapters/part{index}.xhtml")
        };
        if let Some(title) = title {
            navigation.push_str(&format!(r#"<li><a href="{href}">{title}</a></li>"#));
            points.push_str(&format!(r#"<navPoint id="point{index}" playOrder="{}"><navLabel><text>{title}</text></navLabel><content src="{href}"/></navPoint>"#, index + 1));
        }
    }
    opf = opf
        .replace("</manifest>", &format!("{manifest}</manifest>"))
        .replace("</spine>", &format!("{spine}</spine>"));
    let nav = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body><nav epub:type="toc"><ol>{navigation}</ol></nav></body></html>"#
    );
    let ncx = format!(
        r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="test"/></head><docTitle><text>Fixture</text></docTitle><navMap>{points}</navMap></ncx>"#
    );
    let resources = [
        ("mimetype", "application/epub+zip"),
        (
            "META-INF/container.xml",
            r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        ),
        ("OEBPS/package.opf", opf.as_str()),
        ("OEBPS/chapters/one.xhtml", chapter),
        ("OEBPS/notes.xhtml", notes),
        ("OEBPS/nav.xhtml", nav.as_str()),
        ("OEBPS/toc.ncx", ncx.as_str()),
    ];
    for (name, body) in resources {
        archive.start_file(name, options).unwrap();
        archive.write_all(body.as_bytes()).unwrap();
    }
    for index in 1..titles.len() {
        archive
            .start_file(format!("OEBPS/chapters/part{index}.xhtml"), options)
            .unwrap();
        archive.write_all(chapter.as_bytes()).unwrap();
    }
    archive.finish().unwrap();
}

const CHAPTER: &str = r##"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Hidden title</title><style>.hidden {}</style></head><body><p>First &#65; &amp; second [12].</p><p>Unicode 中文 😀.</p><p>Read<a epub:type="noteref" href="../notes.xhtml#note%201">1</a> and <a role="doc-noteref" href="#local">2</a>, repeat <a epub:type="noteref" href="../notes.xhtml#note%201">1</a>, missing <a epub:type="noteref" href="../notes.xhtml#missing">3</a>, remote <a epub:type="noteref" href="https://example.invalid/notes#note">4</a>.</p><aside epub:type="footnote" id="local"><p>Local note body.</p></aside><script>DO_NOT_SHOW</script></body></html>"##;
const NOTES: &str = r##"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Notes</title></head><body><aside epub:type="endnote" id="note 1"><p>The actual endnote text 中文.</p><p>A second paragraph &amp; entity.</p><a role="doc-backlink" href="chapters/one.xhtml#ref1">↩</a></aside></body></html>"##;

#[test]
fn names_untitled_spine_parts_as_numbered_continuations() {
    let directory = tempfile::tempdir().unwrap();
    for version in ["2.0", "3.0"] {
        let path = directory
            .path()
            .join(format!("continuations-{version}.epub"));
        fixture_with_titles(
            &path,
            version,
            CHAPTER,
            NOTES,
            &[
                None,
                None,
                Some("  第一章  "),
                None,
                Some("  "),
                Some(" nUlL "),
                Some("第二章"),
                None,
            ],
        );
        let toc = parser::extract_toc(&path).unwrap();
        let english = toc
            .iter()
            .map(|entry| entry.display_label("continued"))
            .collect::<Vec<_>>();
        assert_eq!(
            english,
            [
                "Chapter 1",
                "Chapter 1 (continued)",
                "第一章",
                "第一章 (continued)",
                "第一章 (continued 2)",
                "第一章 (continued 3)",
                "第二章",
                "第二章 (continued)",
            ]
        );
        assert_eq!(toc[4].display_label("续"), "第一章 (续 2)");
        assert_eq!(
            toc.iter().map(|entry| entry.index).collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
    }
}

#[test]
#[ignore = "Set FNINE_TEST_BOOKS to a directory of local EPUBs; reads without changing library data"]
fn local_epub_endnote_regression() {
    let directory = std::env::var("FNINE_TEST_BOOKS").expect("Set FNINE_TEST_BOOKS");
    let mut books = 0;
    let mut unresolved = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "epub") {
            continue;
        }
        parser::validate_archive(&path).unwrap();
        let epub = rbook::Epub::open(&path).unwrap();
        let mut notes = 0;
        let mut references = 0;
        for (index, entry) in epub.spine().iter().enumerate() {
            let resource = entry.manifest_entry().unwrap();
            let html = resource.read_str().unwrap();
            let content =
                crate::epub::content::extract(&epub, resource.href().path().as_str(), &html)
                    .unwrap();
            let without_references = html
                .replace("<sup", "<span")
                .replace("</sup", "</span")
                .replace("math-super", "baseline")
                .replace("href=", "data-original-href=");
            let baseline = crate::epub::content::extract(
                &epub,
                resource.href().path().as_str(),
                &without_references,
            )
            .unwrap();
            assert_eq!(
                content.text(),
                baseline.text(),
                "Chapter coordinates changed in {} chapter {index}",
                path.display()
            );
            references += content
                .runs
                .iter()
                .filter(|run| !run.note_key.is_empty())
                .count();
            notes += content.notes.len();
            for note in content.notes.iter().filter(|note| !note.available) {
                unresolved.push(format!(
                    "{} chapter {index} ({}): {}",
                    path.file_name().unwrap().to_string_lossy(),
                    resource.href().path(),
                    note.label
                ));
            }
        }
        println!(
            "{}: {references} references, {notes} notes",
            path.file_name().unwrap().to_string_lossy()
        );
        assert!(notes > 0, "No notes recognized in {}", path.display());
        books += 1;
    }
    assert!(books > 0);
    assert!(
        unresolved.is_empty(),
        "Unresolved notes: {}",
        unresolved.join(", ")
    );
}

async fn setup() -> (tempfile::TempDir, AppState, std::fs::File, String) {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::new(Config {
        host: "127.0.0.1".into(),
        port: 0,
        data_dir: directory.path().to_path_buf(),
    });
    let lock = state.initialize().await.unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    fixture(
        &state.books_dir().join(format!("{id}.epub")),
        "3.0",
        CHAPTER,
        NOTES,
    );
    let mut data = state.books.load().await.unwrap();
    data.books.push(Book {
        id: id.clone(),
        title: "Fixture Book".into(),
        author: "Test Author".into(),
        cover_ext: None,
        chapter_count: 1,
        file_name: "fixture.epub".into(),
        file_size: 1,
        uploaded_at: chrono::Utc::now(),
        normalization_version: 2,
    });
    state.books.save(&data).await.unwrap();
    (directory, state, lock, id)
}

async fn request(state: &AppState, method: &str, uri: &str, body: serde_json::Value) -> Response {
    crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn body(response: Response) -> String {
    String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

#[test]
fn extracts_text_and_resolves_actual_endnotes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("book.epub");
    for version in ["2.0", "3.0"] {
        fixture(&path, version, CHAPTER, NOTES);
        let chapter = parser::read_chapter(&path, 0).unwrap();
        let text = chapter.text();
        assert!(text.contains("First A & second [12].\n"), "{text:?}");
        assert!(text.contains("Unicode 中文 😀."));
        assert!(!text.contains("DO_NOT_SHOW") && !text.contains("Hidden title"));
        assert_eq!(chapter.notes.len(), 4);
        assert!(
            chapter.notes[0]
                .text
                .contains("The actual endnote text 中文.\nA second paragraph & entity.")
        );
        assert!(!chapter.notes[0].text.contains('↩'));
        assert_eq!(chapter.notes[1].text, "Local note body.");
        assert!(!chapter.notes[2].available && !chapter.notes[3].available);
        assert_eq!(
            chapter
                .runs
                .iter()
                .filter(|run| run.note_key == "note-0")
                .count(),
            2
        );
        assert_eq!(
            parser::extract_toc(&path).unwrap()[0].label,
            "Actual chapter title"
        );
    }
}

#[test]
fn resolves_conventional_named_targets_without_following_cycles() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("book.epub");
    fixture(
        &path,
        "2.0",
        r##"<html><body><p>Text<a href="../notes.xhtml#fn1"><sup>1</sup></a><a href="#ordinary"><sup>8</sup></a></p><p id="ordinary">Ordinary section</p></body></html>"##,
        r##"<html><body><p><a name="fn1"></a>Named note text.<a role="doc-backlink" href="chapters/one.xhtml">↩</a></p></body></html>"##,
    );
    let chapter = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(chapter.notes.len(), 1);
    assert_eq!(chapter.notes[0].text, "Named note text.");
}

#[test]
fn resolves_publisher_styled_and_plain_numeric_reciprocal_notes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("publisher.epub");
    let chapter = r##"<html><body><p>Text<span class="math-super"><a id="z17" href="../notes.xhtml#bz17">17</a></span> and <a id="z18" href="../notes.xhtml#bz18">18</a>.</p><p><a href="#section">19</a></p><h2 id="section">An ordinary section</h2></body></html>"##;
    let notes = r##"<html><body><p class="zhusi"><a id="bz17" href="chapters/one.xhtml#z17">[17]</a> Publisher note body.</p><p><a id="bz18" href="chapters/one.xhtml#z18">[18]</a> Another full note.</p></body></html>"##;
    fixture(&path, "2.0", chapter, notes);
    let epub = rbook::Epub::open(&path).unwrap();
    let content =
        crate::epub::content::extract(&epub, "/OEBPS/chapters/one.xhtml", chapter).unwrap();
    assert_eq!(content.notes.len(), 2);
    assert_eq!(content.notes[0].text, "Publisher note body.");
    assert_eq!(content.notes[1].text, "Another full note.");
    assert_eq!(content.notes[0].label, "17");
    assert_eq!(content.notes[1].label, "18");
    let reverse = crate::epub::content::extract(&epub, "/OEBPS/notes.xhtml", notes).unwrap();
    assert!(
        reverse.notes.is_empty(),
        "Backlinks must not become note references"
    );
}

#[test]
fn resolves_unlinked_circled_notes_in_local_groups_without_changing_text() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unlinked.epub");
    let chapter = "<html><body><p>First<sup><small><span>①</span></small></sup> and second<sup>②</sup>.</p><ul><li>① First local note.</li><li>② Second local note.</li></ul><p>Later<sup>①</sup>.</p><ul><li>① Different local note.</li></ul><p>Formula x<sup>2</sup> and missing<sup>③</sup>.</p></body></html>";
    fixture(&path, "2.0", chapter, NOTES);
    let content = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(content.notes.len(), 4);
    assert!(content.notes[0].text.contains("First local note."));
    assert!(content.notes[1].text.contains("Second local note."));
    assert!(content.notes[2].text.contains("Different local note."));
    assert_ne!(content.notes[0].key, content.notes[2].key);
    assert!(!content.notes[3].available);
    assert_eq!(
        content.text(),
        "First① and second②.\n① First local note.\n② Second local note.\nLater①.\n① Different local note.\nFormula x2 and missing③.\n"
    );
}

#[test]
fn does_not_guess_ambiguous_unlinked_note_groups() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ambiguous.epub");
    fixture(
        &path,
        "2.0",
        "<html><body><p>One<sup>①</sup>, two<sup>②</sup>.</p><ul><li>① Incomplete list.</li></ul><h2>Next section</h2><ul><li>② Unrelated list.</li></ul></body></html>",
        NOTES,
    );
    let content = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(content.notes.len(), 2);
    assert!(content.notes.iter().all(|note| !note.available));
}

#[test]
fn resolves_misprinted_circled_labels_using_explicit_list_values() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("misprinted.epub");
    fixture(
        &path,
        "2.0",
        "<html><body><p>Birds<sup>①</sup><sup>②</sup><sup>③</sup><sup>④</sup>.</p><ul><li value=\"1\">① First bird.</li><li value=\"2\">② Second bird.</li><li value=\"3\">② Third bird.</li><li value=\"4\">③ Fourth bird.</li></ul></body></html>",
        NOTES,
    );
    let content = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(content.notes.len(), 4);
    assert_eq!(content.notes[2].text, "Third bird.");
    assert_eq!(content.notes[3].text, "Fourth bird.");
}

#[test]
fn resolves_one_unlinked_note_split_into_adjacent_numbered_items() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("split-note.epub");
    fixture(
        &path,
        "2.0",
        "<html><body><p>Phrase<sup>①</sup>.</p><div>&nbsp;</div><ul><li value=\"1\">Translation A: first explanation.</li><li value=\"2\">Translation B: alternative explanation.</li></ul><p>Unknown<sup>①</sup>.</p><ul><li>Unnumbered unrelated list.</li></ul></body></html>",
        NOTES,
    );
    let content = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(content.notes.len(), 2);
    assert_eq!(
        content.notes[0].text,
        "Translation A: first explanation.\nTranslation B: alternative explanation."
    );
    assert!(!content.notes[1].available);
}

#[test]
fn bounds_archive_resources() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large.epub");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    archive
        .start_file("large.xhtml", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive
        .write_all(&vec![b' '; 16 * 1024 * 1024 + 1])
        .unwrap();
    archive.finish().unwrap();
    assert!(matches!(
        parser::validate_archive(&path),
        Err(crate::error::AppError::TooLarge)
    ));
}

#[test]
fn resolves_marker_ids_to_full_note_paragraph_not_just_number() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.epub");
    fixture(
        &path,
        "3.0",
        r##"<html xmlns:epub="http://www.idpf.org/2007/ops"><body><p>Body<a epub:type="noteref" href="../notes.xhtml#note1">1</a></p></body></html>"##,
        r##"<html><body><p><span id="note1">1</span> This is the full note, not just its marker.</p></body></html>"##,
    );
    let chapter = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(
        chapter.notes[0].text,
        "1 This is the full note, not just its marker."
    );
}

#[test]
fn validates_positions_and_resource_targets() {
    for size in 4..12 {
        let mut bytes = vec![b'0'; size];
        bytes[..4].copy_from_slice(b"RIFF");
        assert!(parser::infer_extension(&bytes).is_none());
    }
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(crate::validation::position(value).is_err());
    }
    assert!(crate::validation::position(0.5).is_ok());
    assert!(crate::validation::id("not-an-id").is_err());
    assert!(
        crate::epub::content::resolve_target("/chapter.xhtml", "../outside.xhtml#note").is_none()
    );
    assert!(crate::epub::content::resolve_target("/chapter.xhtml", "file:///note#note").is_none());
}

#[test]
fn evicts_cache_by_weight_and_recent_use() {
    let cache = crate::cache::Cache::with_capacity(5);
    cache.insert_weighted("first", 1, 2);
    cache.insert_weighted("second", 2, 2);
    assert_eq!(cache.get(&"first"), Some(1));
    cache.insert_weighted("third", 3, 3);
    assert_eq!(cache.get(&"second"), None);
    assert_eq!(cache.get(&"first"), Some(1));
    cache.insert_weighted("large", 4, 6);
    assert_eq!(cache.get(&"large"), None);
}

#[tokio::test]
async fn upgrades_legacy_coordinates_with_backup_and_review_flags() {
    let (directory, state, _lock, id) = setup().await;
    let mut books = state.books.load().await.unwrap();
    books.books[0].normalization_version = 1;
    state.books.save(&books).await.unwrap();
    let annotation = crate::db::annotations::Annotation {
        id: uuid::Uuid::new_v4().to_string(),
        book_id: id.clone(),
        chapter: 0,
        selected_text: "Unicode 中文 😀.".into(),
        note: Some("keep me".into()),
        position_start: 0.1,
        position_end: 0.2,
        color: "#ffee77".into(),
        created_at: chrono::Utc::now(),
        needs_review: false,
        prefix: String::new(),
        suffix: String::new(),
    };
    let mut unresolved = annotation.clone();
    unresolved.id = uuid::Uuid::new_v4().to_string();
    unresolved.selected_text = "text that cannot be located".into();
    state
        .annotations_store(&id)
        .save(&crate::db::annotations::AnnotationsData {
            annotations: vec![annotation, unresolved],
        })
        .await
        .unwrap();
    crate::epub::migration::upgrade(&state, &books.books[0])
        .await
        .unwrap();
    assert_eq!(
        state.books.load().await.unwrap().books[0].normalization_version,
        2
    );
    let annotations = state
        .annotations_store(&id)
        .load()
        .await
        .unwrap()
        .annotations;
    assert!(!annotations[0].needs_review && annotations[1].needs_review);
    assert_eq!(annotations[0].note.as_deref(), Some("keep me"));
    assert!(
        directory
            .path()
            .join("backups")
            .join(format!("{id}-normalization-v1.toml"))
            .is_file()
    );
}

#[tokio::test]
async fn recovers_pending_deletion_and_uncommitted_import() {
    let (directory, state, lock, id) = setup().await;
    Store::<crate::db::books::BooksData>::new(
        directory
            .path()
            .join("deletions")
            .join(format!("{id}.toml")),
    )
    .save(&Default::default())
    .await
    .unwrap();
    let orphan = uuid::Uuid::new_v4().to_string();
    tokio::fs::write(state.books_dir().join(format!("{orphan}.epub")), "partial")
        .await
        .unwrap();
    Store::<crate::db::books::BooksData>::new(
        directory
            .path()
            .join("imports")
            .join(format!("{orphan}.toml")),
    )
    .save(&Default::default())
    .await
    .unwrap();
    drop(lock);
    let restarted = AppState::new(state.config.clone());
    let _new_lock = restarted.initialize().await.unwrap();
    assert!(restarted.books.load().await.unwrap().books.is_empty());
    assert!(
        !restarted
            .books_dir()
            .join(format!("{orphan}.epub"))
            .exists()
    );
    assert!(!restarted.books_dir().join(format!("{id}.epub")).exists());
}

fn multipart(name: &str, contents: &[u8]) -> Vec<u8> {
    let mut data = format!("--fixture\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
    data.extend_from_slice(contents);
    data.extend_from_slice(b"\r\n--fixture--\r\n");
    data
}

#[tokio::test]
async fn streams_uploads_cleans_invalid_imports_and_confines_fonts() {
    let (_directory, state, _lock, id) = setup().await;
    let invalid = crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload")
                .header("content-type", "multipart/form-data; boundary=fixture")
                .body(Body::from(multipart("broken.epub", b"not an EPUB")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(std::fs::read_dir(state.books_dir()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_dir(state.config.data_dir.join("staging"))
            .unwrap()
            .count(),
        0
    );
    let epub = tokio::fs::read(state.books_dir().join(format!("{id}.epub")))
        .await
        .unwrap();
    let uploaded = crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload")
                .header("content-type", "multipart/form-data; boundary=fixture")
                .body(Body::from(multipart("valid.epub", &epub)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.status(), StatusCode::SEE_OTHER);
    assert_eq!(state.books.load().await.unwrap().books.len(), 2);
    let font = crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/settings/fonts")
                .header("content-type", "multipart/form-data; boundary=fixture")
                .body(Body::from(multipart(
                    "display-name.ttf",
                    b"\0\x01\0\0fixture",
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(font.status(), StatusCode::SEE_OTHER);
    let inventory = state.fonts.load().await.unwrap();
    assert_ne!(inventory.fonts[0].id, "display-name.ttf");
    let css = request(
        &state,
        "GET",
        &format!("/settings/fonts/{}/style.css", inventory.fonts[0].id),
        serde_json::Value::Null,
    )
    .await;
    assert!(body(css).await.contains("font-family: FnineCustom"));
    let rejected = crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/settings/fonts/delete")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("name=unknown.ttf"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert!(state.fonts_dir().join(&inventory.fonts[0].id).is_file());
}

#[tokio::test]
async fn ignores_orphan_progress_and_keeps_cache_fresh() {
    let (_directory, state, _lock, id) = setup().await;
    state.load_progress().await.unwrap();
    assert_eq!(
        request(
            &state,
            "POST",
            "/api/progress",
            serde_json::json!({"book_id":id,"chapter":0,"position":0.7})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        state.load_progress().await.unwrap().entries[&id].position,
        0.7
    );
    let mut progress = state.progress.load().await.unwrap();
    let mut orphan = progress.entries[&id].clone();
    orphan.book_id = uuid::Uuid::new_v4().to_string();
    orphan.updated_at += chrono::Duration::seconds(10);
    progress.entries.insert(orphan.book_id.clone(), orphan);
    state.progress.save(&progress).await.unwrap();
    state.invalidate_progress_cache();
    assert_eq!(
        request(&state, "GET", "/api/recent", serde_json::Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
}

#[test]
#[ignore]
fn browser_fixture() {
    let destination = std::env::var("FNINE_TEST_EPUB")
        .expect("Set FNINE_TEST_EPUB to the browser fixture destination");
    let paragraphs = (0..300)
        .map(|index| {
            format!("<p>Paragraph {index}: reliable reading across devices and notes. 中文 😀.</p>")
        })
        .collect::<String>();
    let chapter = CHAPTER.replace("</body>", &format!(r##"<p>Publisher<span class="math-super"><a id="z17" href="../notes.xhtml#bz17">17</a></span>.</p><p>Unlinked<sup><small>①</small></sup>.</p><ul><li>① The first unlinked note body.</li></ul><p>Repeated<sup>①</sup>.</p><ul><li>① A different unlinked note body.</li></ul>{paragraphs}</body>"##));
    let chapter = chapter.replace("</body>", r##"<p>😀<a epub:type="noteref" href="../notes.xhtml#note%201">1</a><a epub:type="noteref" href="../notes.xhtml#note%201">1</a>.</p><p><a epub:type="noteref" href="../notes.xhtml#note%201">1</a> Paragraph-start note.</p></body>"##);
    let notes = NOTES.replace(
        "</aside>",
        &format!(
            "<p>{}</p></aside>",
            "Long note text for popup scrolling. ".repeat(300)
        ),
    ).replace("</body>", r##"<p class="zhusi"><a id="bz17" href="chapters/one.xhtml#z17">[17]</a> The actual publisher-styled note body.</p></body>"##);
    fixture_with_titles(
        std::path::Path::new(&destination),
        "3.0",
        &chapter,
        &notes,
        &[
            Some("Actual chapter title"),
            None,
            Some(" null "),
            Some("Next chapter"),
            None,
        ],
    );
}

#[tokio::test]
async fn locks_directory_and_preserves_corrupt_store() {
    let (directory, state, _lock, _) = setup().await;
    assert!(
        AppState::new(state.config.clone())
            .initialize()
            .await
            .is_err()
    );
    let path = directory.path().join("broken.toml");
    tokio::fs::write(&path, "[broken").await.unwrap();
    assert!(
        Store::<crate::db::books::BooksData>::new(&path)
            .load()
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "[broken");
    assert!(
        Store::<Vec<String>>::new(&path)
            .save(&vec!["unsupported root".into()])
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "[broken");
}

#[test]
fn keeps_duplicate_labels_distinct_and_bounds_long_notes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("book.epub");
    let chapter = r##"<html xmlns:epub="http://www.idpf.org/2007/ops"><body><p>Text<a epub:type="noteref" href="../notes.xhtml#one">1</a><a epub:type="noteref" href="../notes.xhtml#two">1</a></p></body></html>"##;
    let notes = format!(
        "<html><body><aside id=\"one\"><p>{}</p></aside><aside id=\"two\"><p>Different note body.</p></aside></body></html>",
        "Long note paragraph. ".repeat(1000)
    );
    fixture(&path, "3.0", chapter, &notes);
    let content = parser::read_chapter(&path, 0).unwrap();
    assert_eq!(content.notes.len(), 2);
    assert_ne!(content.notes[0].key, content.notes[1].key);
    assert!(content.notes[0].text.len() > 10_000);
    assert_eq!(content.notes[1].text, "Different note body.");
}

#[tokio::test]
async fn settings_validation_and_library_filters() {
    let (_directory, state, _lock, id) = setup().await;
    let response = crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/settings")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "language=en&theme=dark&font_size=200&font_family=",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(state.settings.load().await.unwrap().font_size, 18);
    let unread = body(
        request(
            &state,
            "GET",
            "/?status=unread&lang=en",
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert!(unread.contains("Fixture Book"));
    assert!(!unread.contains("name=\"q\""));
    assert!(!unread.contains("href=\"/search"));
    assert!(unread.contains("id=\"library-sort\""));
    assert!(unread.contains("id=\"library-status\""));
    assert_eq!(
        request(&state, "GET", "/search", serde_json::Value::Null)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    request(
        &state,
        "POST",
        "/api/progress",
        serde_json::json!({"book_id":id,"chapter":0,"position":1.0}),
    )
    .await;
    let finished = body(
        request(
            &state,
            "GET",
            "/?status=finished&lang=en",
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert!(finished.contains("Fixture Book"));
    let unread = body(
        request(
            &state,
            "GET",
            "/?status=unread&lang=en",
            serde_json::Value::Null,
        )
        .await,
    )
    .await;
    assert!(!unread.contains("class=\"book-card\""));
}

#[cfg(unix)]
#[tokio::test]
async fn refuses_symlink_fonts_and_store_paths() {
    let (directory, state, _lock, _) = setup().await;
    let outside = directory.path().join("outside.ttf");
    tokio::fs::write(&outside, "secret").await.unwrap();
    let id = format!("{}.ttf", uuid::Uuid::new_v4());
    std::os::unix::fs::symlink(&outside, state.fonts_dir().join(&id)).unwrap();
    state
        .fonts
        .save(&crate::db::fonts::FontsData {
            fonts: vec![crate::db::fonts::Font {
                id: id.clone(),
                name: "external".into(),
            }],
        })
        .await
        .unwrap();
    assert!(
        crate::handlers::api_settings::known_font(&state, &id)
            .await
            .is_err()
    );
    assert_eq!(
        request(
            &state,
            "GET",
            &format!("/fonts/{id}"),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let linked = directory.path().join("linked.toml");
    std::os::unix::fs::symlink(&outside, &linked).unwrap();
    assert!(
        Store::<crate::db::books::BooksData>::new(linked)
            .load()
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reader_renders_note_text_and_saved_preferences() {
    let (_directory, state, _lock, id) = setup().await;
    let mut settings = state.settings.load().await.unwrap();
    settings.language = "en".into();
    settings.theme = "dark".into();
    state.settings.save(&settings).await.unwrap();
    let response = request(
        &state,
        "GET",
        &format!("/book/{id}/read/0"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body(response).await;
    assert!(html.contains("The actual endnote text 中文."));
    assert!(
        html.contains("data-note=\"note-0\"") && html.contains("<dialog id=\"endnote-dialog\"")
    );
    assert!(html.contains("lang=\"en\"") && html.contains("data-theme=\"dark\""));
}

#[tokio::test]
async fn serializes_concurrent_annotation_updates_and_validates_api() {
    let (_directory, state, _lock, id) = setup().await;
    let mut tasks = Vec::new();
    for index in 0..12 {
        let state = state.clone();
        let id = id.clone();
        tasks.push(tokio::spawn(async move {
            request(&state, "POST", &format!("/api/book/{id}/annotations"), serde_json::json!({"chapter":0,"selected_text":format!("selection {index}"),"position_start":0.1,"position_end":0.2})).await.status()
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap(), StatusCode::OK);
    }
    assert_eq!(
        state
            .annotations_store(&id)
            .load()
            .await
            .unwrap()
            .annotations
            .len(),
        12
    );
    let response = request(
        &state,
        "POST",
        "/api/progress",
        serde_json::json!({"book_id":id,"chapter":5,"position":0.5}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = request(
        &state,
        "POST",
        "/api/progress",
        serde_json::json!({"book_id":id,"chapter":0,"position":1.5}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deletes_associated_state_and_recovers_journal() {
    let (_directory, state, _lock, id) = setup().await;
    assert_eq!(
        request(
            &state,
            "POST",
            "/api/progress",
            serde_json::json!({"book_id":id,"chapter":0,"position":0.5})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            &state,
            "POST",
            &format!("/book/{id}/delete"),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    assert!(!state.books_dir().join(format!("{id}.epub")).exists());
    assert!(
        !state
            .progress
            .load()
            .await
            .unwrap()
            .entries
            .contains_key(&id)
    );
    assert_eq!(
        request(&state, "GET", "/api/recent", serde_json::Value::Null)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn rejects_cross_origin_and_large_json() {
    let (_directory, state, _lock, _) = setup().await;
    let response = crate::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/settings")
                .header("host", "localhost:3000")
                .header("origin", "https://other.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = request(
        &state,
        "POST",
        "/api/progress",
        serde_json::json!({"large":"x".repeat(256*1024)}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn exports_and_edits_annotations() {
    let (_directory, state, _lock, id) = setup().await;
    let response = request(&state, "POST", &format!("/api/book/{id}/annotations"), serde_json::json!({"chapter":0,"selected_text":"First A","note":"original","position_start":0.0,"position_end":0.1})).await;
    let annotation: serde_json::Value = serde_json::from_str(&body(response).await).unwrap();
    let annotation_id = annotation["id"].as_str().unwrap();
    assert_eq!(
        request(
            &state,
            "PATCH",
            &format!("/api/book/{id}/annotations/{annotation_id}"),
            serde_json::json!({"note":"updated note","color":"#a6d8ff"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let response = request(
        &state,
        "GET",
        &format!("/book/{id}/annotations/export"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body(response).await.contains("updated note"));
}
