use crate::run_test_impl;
use std::path::Path;
use svg2pdf::Options;

fn run_api_test(svg_path: &Path, test_name: &str, options: Options) {
    assert_eq!(
        run_test_impl(
            Path::new(svg_path),
            Path::new(&format!("ref/api/{}.png", test_name)),
            Path::new(&format!("diff/api/{}.png", test_name)),
            Path::new(&format!("pdf/api/{}.pdf", test_name)),
            options
        ),
        0
    );
}

#[test]
fn text_to_paths() {
    let options = Options { embed_text: false, ..Options::default() };

    run_api_test(
        Path::new("svg/resvg/text/text/simple-case.svg"),
        "text_to_paths",
        options,
    );
}
