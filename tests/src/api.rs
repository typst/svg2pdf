#[allow(unused_imports)]
use {
    crate::FONTDB,
    crate::render_pdf,
    crate::{convert_svg, run_test_impl},
    pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str},
    std::collections::HashMap,
    std::path::Path,
    svg2pdf::ConversionOptions,
    svg2pdf::PageOptions,
};

#[test]
fn text_to_paths() {
    let options = ConversionOptions { embed_text: false, ..ConversionOptions::default() };

    let svg_path = "svg/resvg/text/text/simple-case.svg";
    let (pdf, actual_image) =
        convert_svg(Path::new(svg_path), options, PageOptions::default());
    let res = run_test_impl(pdf, actual_image, "api/text_to_paths");
    assert_eq!(res, 0);
}

#[test]
fn dpi() {
    let conversion_options = ConversionOptions::default();
    let page_options = PageOptions { dpi: 140.0 };

    let svg_path = "svg/resvg/text/text/simple-case.svg";
    let (pdf, actual_image) =
        convert_svg(Path::new(svg_path), conversion_options, page_options);
    let res = run_test_impl(pdf, actual_image, "api/dpi");
    assert_eq!(res, 0);
}

#[test]
fn to_chunk() {
    let mut alloc = Ref::new(1);
    let catalog_id = alloc.bump();
    let page_tree_id = alloc.bump();
    let page_id = alloc.bump();
    let font_id = alloc.bump();
    let content_id = alloc.bump();
    let font_name = Name(b"F1");
    let svg_name = Name(b"S1");

    let path =
        "svg/custom/integration/wikimedia/coat_of_the_arms_of_edinburgh_city_council.svg";
    let svg = std::fs::read_to_string(path).unwrap();
    let mut options = svg2pdf::usvg::Options::default();
    options.fontdb = FONTDB.clone();
    let tree = svg2pdf::usvg::Tree::from_str(&svg, &options).unwrap();
    let (svg_chunk, svg_id) =
        svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default(), &FONTDB.as_ref());

    let mut map = HashMap::new();
    let svg_chunk =
        svg_chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
    let svg_id = map.get(&svg_id).unwrap();

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id).kids([page_id]).count(1);

    let mut page = pdf.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    page.parent(page_tree_id);
    page.contents(content_id);

    let mut resources = page.resources();
    resources.x_objects().pair(svg_name, svg_id);
    resources.fonts().pair(font_name, font_id);
    resources.finish();
    page.finish();

    pdf.type1_font(font_id).base_font(Name(b"Times-Roman"));

    let mut content = Content::new();

    content
        .transform([300.0, 0.0, 0.0, 300.0, 200.0, 400.0])
        .x_object(svg_name);

    pdf.stream(content_id, &content.finish());
    pdf.extend(&svg_chunk);
    let pdf = pdf.finish();

    let actual_image = render_pdf(pdf.as_slice());
    let res = run_test_impl(pdf, actual_image, "api/to_chunk");

    assert_eq!(res, 0);
}
