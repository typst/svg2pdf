//! Tests for [`svg2pdf::trees_to_pdf`], the multi-page conversion entry point.
//!
//! These focus on behaviour unique to the multi-page path: page count, per-page
//! sizing, and — the whole point of the feature — deduplication of fonts and
//! color profiles across pages. The single-tree path is exercised
//! comprehensively by the visual regression suite in `render.rs`, since
//! `to_pdf` now delegates to `trees_to_pdf`.

// These helpers and imports are only referenced from `#[test]` functions, which
// are absent from the plain (non-test) `cargo build --all` that CI runs with
// `-Dwarnings`. Allow them so that build stays warning-free, as `api.rs` and
// `render.rs` do.
#![allow(dead_code, unused_imports)]

use {
    crate::{pdf_page_count, read_svg, render_pdf_page},
    image::RgbaImage,
    std::sync::Arc,
    svg2pdf::{ConversionError, ConversionOptions, PageOptions},
};

/// Builds a minimal SVG string with a single line of text in a given font.
fn text_svg(width: u32, height: u32, font_family: &str, text: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
            <rect x="0" y="0" width="{width}" height="{height}" fill="white"/>
            <text x="10" y="40" font-family="{font_family}" font-size="24" fill="black">{text}</text>
        </svg>"#
    )
}

/// Counts non-overlapping byte occurrences of `needle` in `haystack`.
fn count(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || needle.len() > haystack.len() {
        return 0;
    }
    haystack.windows(needle.len()).filter(|w| *w == needle).count()
}

/// Extracts every `/MediaBox [..]` entry from a PDF as raw byte slices.
fn media_boxes(pdf: &[u8]) -> Vec<Vec<u8>> {
    const KEY: &[u8] = b"/MediaBox";
    let mut boxes = Vec::new();
    let mut offset = 0;
    while let Some(rel) = pdf[offset..].windows(KEY.len()).position(|w| w == KEY) {
        let start = offset + rel;
        let Some(close) = pdf[start..].iter().position(|&b| b == b']') else { break };
        boxes.push(pdf[start..=start + close].to_vec());
        offset = start + close + 1;
    }
    boxes
}

/// Whether a rendered page contains any drawn (non-transparent) pixel. The
/// pdfium render config clears to transparent white, so drawn content shows up
/// as a pixel with a non-zero alpha channel.
fn has_content(image: &RgbaImage) -> bool {
    image.pixels().any(|p| p.0[3] != 0)
}

#[test]
fn two_trees_produce_a_two_page_pdf() {
    let a = read_svg(&text_svg(100, 100, "Noto Sans", "Hello"));
    let b = read_svg(&text_svg(120, 100, "Noto Sans", "World"));

    let pdf = svg2pdf::trees_to_pdf(
        &[&a, &b],
        ConversionOptions::default(),
        PageOptions::default(),
    )
    .unwrap();

    // The page tree declares two kids and there are two page objects.
    assert_eq!(count(&pdf, b"/Count 2"), 1);
    assert_eq!(media_boxes(&pdf).len(), 2);
    // pdfium agrees it is a valid two-page document.
    assert_eq!(pdf_page_count(&pdf), 2);
}

#[test]
fn fonts_are_shared_across_pages() {
    // Both pages use the same font but different glyphs, so a single union
    // subset must serve the whole document.
    let a = read_svg(&text_svg(200, 100, "Noto Sans", "Hello"));
    let b = read_svg(&text_svg(200, 100, "Noto Sans", "World Foo"));

    let combined = svg2pdf::trees_to_pdf(
        &[&a, &b],
        ConversionOptions::default(),
        PageOptions::default(),
    )
    .unwrap();

    // Exactly one embedded font program and one font descriptor for the whole
    // document, not one per page.
    assert_eq!(count(&combined, b"/FontFile"), 1);
    assert_eq!(count(&combined, b"/Type /FontDescriptor"), 1);

    // Sanity check that stapling the single-page PDFs together really would
    // duplicate the font: each standalone PDF embeds its own copy.
    let single_a =
        svg2pdf::to_pdf(&a, ConversionOptions::default(), PageOptions::default())
            .unwrap();
    let single_b =
        svg2pdf::to_pdf(&b, ConversionOptions::default(), PageOptions::default())
            .unwrap();
    assert_eq!(count(&single_a, b"/FontFile") + count(&single_b, b"/FontFile"), 2);

    // And the shared document is smaller than the two standalone PDFs combined.
    assert!(combined.len() < single_a.len() + single_b.len());
}

#[test]
fn color_profile_is_shared_across_pages() {
    let a = read_svg(&text_svg(100, 100, "Noto Sans", "Hello"));
    let b = read_svg(&text_svg(100, 100, "Noto Sans", "World"));

    let combined = svg2pdf::trees_to_pdf(
        &[&a, &b],
        ConversionOptions::default(),
        PageOptions::default(),
    )
    .unwrap();

    // A single sRGB ICC profile object serves both pages. The profile stream is
    // the only 3-channel ICC object (`/N 3`) in the document.
    assert_eq!(count(&combined, b"/N 3"), 1);
    assert_eq!(count(&combined, b"/Range [0 1 0 1 0 1]"), 1);

    // Each standalone PDF carries its own copy, so stapling would double it.
    let single =
        svg2pdf::to_pdf(&a, ConversionOptions::default(), PageOptions::default())
            .unwrap();
    assert_eq!(count(&single, b"/N 3"), 1);
}

#[test]
fn pages_can_have_different_sizes() {
    let a = read_svg(&text_svg(100, 100, "Noto Sans", "A"));
    let b = read_svg(&text_svg(200, 150, "Noto Sans", "B"));

    let pdf = svg2pdf::trees_to_pdf(
        &[&a, &b],
        ConversionOptions::default(),
        PageOptions::default(),
    )
    .unwrap();

    let boxes = media_boxes(&pdf);
    assert_eq!(boxes.len(), 2);
    assert_ne!(
        boxes[0], boxes[1],
        "pages built from different-sized trees should differ"
    );
}

#[test]
fn empty_slice_is_an_error() {
    let result =
        svg2pdf::trees_to_pdf(&[], ConversionOptions::default(), PageOptions::default());
    assert!(matches!(result, Err(ConversionError::EmptyDocument)));
}

#[test]
fn single_tree_produces_one_renderable_page() {
    let tree = read_svg(&text_svg(100, 100, "Noto Sans", "Hi"));

    let pdf = svg2pdf::trees_to_pdf(
        &[&tree],
        ConversionOptions::default(),
        PageOptions::default(),
    )
    .unwrap();

    assert_eq!(pdf_page_count(&pdf), 1);
    assert!(has_content(&render_pdf_page(&pdf, 0)));
}

#[test]
fn both_pages_render() {
    let a = read_svg(&text_svg(100, 100, "Noto Sans", "Page one"));
    let b = read_svg(&text_svg(100, 100, "Noto Sans", "Page two"));

    let pdf = svg2pdf::trees_to_pdf(
        &[&a, &b],
        ConversionOptions::default(),
        PageOptions::default(),
    )
    .unwrap();

    assert!(has_content(&render_pdf_page(&pdf, 0)));
    assert!(has_content(&render_pdf_page(&pdf, 1)));
}

#[test]
fn text_to_paths_mode_embeds_no_fonts() {
    let options = ConversionOptions { embed_text: false, ..ConversionOptions::default() };
    let a = read_svg(&text_svg(100, 100, "Noto Sans", "Hello"));
    let b = read_svg(&text_svg(100, 100, "Noto Sans", "World"));

    let pdf = svg2pdf::trees_to_pdf(&[&a, &b], options, PageOptions::default()).unwrap();

    assert_eq!(pdf_page_count(&pdf), 2);
    // With text flattened to paths, no font program is embedded at all.
    assert_eq!(count(&pdf, b"/FontFile"), 0);
}

#[test]
fn pdfa_mode_succeeds() {
    let options = ConversionOptions { pdfa: true, ..ConversionOptions::default() };
    let a = read_svg(&text_svg(100, 100, "Noto Sans", "Hello"));
    let b = read_svg(&text_svg(100, 100, "Noto Sans", "World"));

    let pdf = svg2pdf::trees_to_pdf(&[&a, &b], options, PageOptions::default()).unwrap();

    assert_eq!(pdf_page_count(&pdf), 2);
    // Fonts are still shared across pages in PDF/A mode.
    assert_eq!(count(&pdf, b"/FontFile"), 1);
}

#[test]
fn trees_from_different_font_databases_error() {
    // Each tree is parsed with its own font database, violating the
    // shared-database invariant that font dedup relies on. `fontdb::ID`s are
    // only unique within one database, so this must be rejected rather than
    // silently rendering the wrong glyphs.
    let separate_tree = |text: &str| {
        let mut db = fontdb::Database::new();
        db.load_fonts_dir("fonts");
        db.set_sans_serif_family("Noto Sans");
        let options = usvg::Options { fontdb: Arc::new(db), ..usvg::Options::default() };
        usvg::Tree::from_str(&text_svg(100, 100, "Noto Sans", text), &options).unwrap()
    };
    let a = separate_tree("Hello");
    let b = separate_tree("World");

    let result = svg2pdf::trees_to_pdf(
        &[&a, &b],
        ConversionOptions::default(),
        PageOptions::default(),
    );
    assert!(matches!(result, Err(ConversionError::MismatchedFontDatabases)));
}
