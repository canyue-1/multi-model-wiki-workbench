use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};
use multimodel_wiki_workbench_lib::repository::WorkspaceRepository;
use multimodel_wiki_workbench_lib::sources::{
    IngestError, SourceIngestor, WorkspaceSourceIngestor,
};
use tempfile::tempdir;
use url::Url;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[tokio::test]
async fn copies_markdown_without_modifying_original() {
    let source_dir = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let source_path = source_dir.path().join("note.md");
    fs::write(&source_path, "# 资料\n\n原始内容").unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let ingestor = WorkspaceSourceIngestor::new(workspace.path(), repo.clone()).unwrap();

    let result = ingestor.ingest_file(&source_path).await.unwrap();

    assert!(result.raw_path.starts_with("raw/"));
    assert_eq!(result.extracted_text.as_deref(), Some("# 资料\n\n原始内容"));
    assert_eq!(
        fs::read_to_string(&source_path).unwrap(),
        "# 资料\n\n原始内容"
    );
    let copied_path = workspace.path().join(&result.raw_path);
    assert_eq!(
        fs::read_to_string(&copied_path).unwrap(),
        "# 资料\n\n原始内容"
    );
    assert!(fs::metadata(copied_path).unwrap().permissions().readonly());
    assert_eq!(repo.load_source(&result.id).await.unwrap(), result);
}

#[tokio::test]
async fn content_hash_keeps_same_named_files_distinct() {
    let first_dir = tempdir().unwrap();
    let second_dir = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let first = first_dir.path().join("note.txt");
    let second = second_dir.path().join("note.txt");
    fs::write(&first, "first").unwrap();
    fs::write(&second, "second").unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let ingestor = WorkspaceSourceIngestor::new(workspace.path(), repo).unwrap();

    let first_result = ingestor.ingest_file(&first).await.unwrap();
    let second_result = ingestor.ingest_file(&second).await.unwrap();

    assert_ne!(first_result.raw_path, second_result.raw_path);
    assert_eq!(
        fs::read_to_string(workspace.path().join(first_result.raw_path)).unwrap(),
        "first"
    );
    assert_eq!(
        fs::read_to_string(workspace.path().join(second_result.raw_path)).unwrap(),
        "second"
    );
}

#[tokio::test]
async fn extraction_failure_keeps_the_raw_file_and_records_the_error() {
    let source_dir = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let source_path = source_dir.path().join("broken.pdf");
    fs::write(&source_path, b"not a pdf").unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let ingestor = WorkspaceSourceIngestor::new(workspace.path(), repo.clone()).unwrap();

    let result = ingestor.ingest_file(&source_path).await.unwrap();

    assert!(result.extracted_text.is_none());
    assert!(
        result
            .extraction_error
            .as_deref()
            .is_some_and(|message| !message.is_empty())
    );
    assert!(workspace.path().join(&result.raw_path).exists());
    assert_eq!(repo.load_source(&result.id).await.unwrap(), result);
}

#[tokio::test]
async fn rejects_unsupported_files_without_touching_the_original() {
    let source_dir = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let source_path = source_dir.path().join("archive.bin");
    fs::write(&source_path, b"opaque").unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let ingestor = WorkspaceSourceIngestor::new(workspace.path(), repo).unwrap();

    let error = ingestor.ingest_file(&source_path).await.unwrap_err();

    assert!(matches!(error, IngestError::UnsupportedFormat(extension) if extension == "bin"));
    assert_eq!(fs::read(&source_path).unwrap(), b"opaque");
    assert!(!workspace.path().join("raw").exists());
}

#[tokio::test]
async fn captures_webpage_html_and_extracts_readable_text() {
    let workspace = tempdir().unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let ingestor = WorkspaceSourceIngestor::new(workspace.path(), repo.clone()).unwrap();
    let (url, server) = serve_once(
        "text/html; charset=utf-8",
        "<html><head><title>示例页面</title></head><body><main><h1>核心结论</h1><p>这是正文。</p></main><script>ignore()</script></body></html>",
    );

    let result = ingestor.capture_url(&url).await.unwrap();
    server.join().unwrap();

    assert_eq!(result.kind, "url");
    assert_eq!(result.title, "示例页面");
    assert!(result.extracted_text.as_deref().is_some_and(|text| {
        text.contains("核心结论") && text.contains("这是正文") && !text.contains("ignore")
    }));
    let snapshot = fs::read_to_string(workspace.path().join(&result.raw_path)).unwrap();
    assert!(snapshot.contains("<title>示例页面</title>"));
    assert_eq!(result.source_uri, url.as_str());
    assert_eq!(repo.load_source(&result.id).await.unwrap(), result);
}

#[tokio::test]
async fn extracts_docx_pdf_and_image_metadata() {
    let source_dir = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let docx = source_dir.path().join("document.docx");
    let pdf = source_dir.path().join("document.pdf");
    let png = source_dir.path().join("pixel.png");
    write_docx(&docx, "DOCX 正文");
    write_pdf(&pdf, "Hello PDF");
    fs::write(&png, ONE_BY_ONE_PNG).unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let ingestor = WorkspaceSourceIngestor::new(workspace.path(), repo).unwrap();

    let docx_result = ingestor.ingest_file(&docx).await.unwrap();
    let pdf_result = ingestor.ingest_file(&pdf).await.unwrap();
    let png_result = ingestor.ingest_file(&png).await.unwrap();

    assert_eq!(docx_result.extracted_text.as_deref(), Some("DOCX 正文"));
    assert!(
        pdf_result
            .extracted_text
            .as_deref()
            .is_some_and(|text| text.contains("Hello PDF"))
    );
    assert!(png_result.extracted_text.as_deref().is_some_and(|text| {
        text.contains("PNG") && text.contains("宽度: 1 px") && text.contains("高度: 1 px")
    }));
}

fn serve_once(content_type: &str, body: &str) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let content_type = content_type.to_owned();
    let body = body.to_owned();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (
        Url::parse(&format!("http://{address}/article")).unwrap(),
        server,
    )
}

fn write_docx(path: &std::path::Path, text: &str) {
    let file = fs::File::create(path).unwrap();
    let mut archive = ZipWriter::new(file);
    archive
        .start_file("word/document.xml", SimpleFileOptions::default())
        .unwrap();
    write!(
        archive,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:body></w:document>"#
    )
    .unwrap();
    archive.finish().unwrap();
}

fn write_pdf(path: &std::path::Path, text: &str) {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = document.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 14.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);
    document.compress();
    document.save(path).unwrap();
}

const ONE_BY_ONE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 0, 0, 3, 1, 1,
    0, 24, 221, 141, 113, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
