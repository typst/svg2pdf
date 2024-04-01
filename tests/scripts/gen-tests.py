#!/usr/bin/env python3
import argparse
import shutil

from common import SVG_DIR, ROOT, TestFile
from pathlib import Path

OUT_PATH = ROOT / "src" / "render.rs"

NO_RELATIVE_PATHS = "no relative paths supported"
INVESTIGATE = "need to investigate"
NO_REFLECT = "spreadMethod reflect not supported"
NO_REPEAT = "spreadMethod repeat not supported"
NO_SUPPORT = "not supported in PDF"
NO_FONT = "font is not part of test suite yet"

IGNORE_TESTS = {
    # The following test cases still need to be investigated
    "svg/resvg/paint-servers/pattern/nested-objectBoundingBox.svg": INVESTIGATE,

    "svg/resvg/painting/stroke-dasharray/n-0.svg": INVESTIGATE,
    "svg/resvg/structure/image/image-with-float-size-scaling.svg": INVESTIGATE,
    "svg/resvg/structure/svg/funcIRI-parsing.svg": INVESTIGATE,
    "svg/resvg/structure/svg/funcIRI-with-invalid-characters.svg": INVESTIGATE,
    "svg/resvg/text/font-weight/bolder-with-clamping.svg": INVESTIGATE,
    "svg/resvg/text/font-weight/lighter-with-clamping.svg": INVESTIGATE,
    "svg/resvg/text/font-weight/lighter-without-parent.svg": INVESTIGATE,
    "svg/resvg/text/letter-spacing/mixed-scripts.svg": INVESTIGATE,
    "svg/resvg/text/text/compound-emojis.svg": INVESTIGATE,
    "svg/resvg/text/text/compound-emojis-and-coordinates-list.svg": INVESTIGATE,
    "svg/resvg/text/text/emojis.svg": INVESTIGATE,

    # The following test cases need to be excluded due to technical reasons
    # and are not considered as wrong.
    "svg/resvg/filters/feMorphology/huge-radius.svg": "will timeout CI",
    "svg/resvg/filters/filter/huge-region.svg": "will sigkill",
    "svg/resvg/structure/svg/negative-size.svg": "invalid size",
    "svg/resvg/structure/svg/no-size.svg": "invalid size",
    "svg/resvg/structure/svg/zero-size.svg": "invalid size",
    "svg/resvg/structure/svg/not-UTF-8-encoding.svg": "invalid encoding",
    "svg/resvg/filters/feImage/simple-case.svg": NO_RELATIVE_PATHS,
    "svg/resvg/painting/marker/with-an-image-child.svg": NO_RELATIVE_PATHS,
    "svg/resvg/painting/mix-blend-mode/color-dodge.svg": "pdfium bug",
    "svg/resvg/painting/stroke-linejoin/miter-clip.svg": NO_SUPPORT,
    "svg/resvg/structure/image/external-gif.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-jpeg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-png.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-svg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-svg-with-transform.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-svgz.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/float-size.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/no-height.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/no-height-on-svg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/no-width.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/no-width-on-svg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/no-width-and-height.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/no-width-and-height-on-svg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/raster-image-and-size-with-odd-numbers.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/recursive-1.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/recursive-2.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/width-and-height-set-to-auto.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/zero-height.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/zero-width.svg": NO_RELATIVE_PATHS,

    # The following test cases should work but are not implemented in svg2pdf yet.
    "svg/resvg/paint-servers/linearGradient/attributes-via-xlink-href-complex-order.svg": NO_REFLECT,
    "svg/resvg/paint-servers/linearGradient/attributes-via-xlink-href-from-radialGradient.svg": NO_REFLECT,
    "svg/resvg/paint-servers/linearGradient/spreadMethod=reflect.svg": NO_REFLECT,
    "svg/resvg/paint-servers/linearGradient/spreadMethod=repeat.svg": NO_REPEAT,
    "svg/resvg/paint-servers/radialGradient/attributes-via-xlink-href-complex-order.svg":NO_REFLECT,
    "svg/resvg/paint-servers/radialGradient/attributes-via-xlink-href-from-linearGradient.svg": NO_REFLECT,
    "svg/resvg/paint-servers/radialGradient/spreadMethod=reflect.svg": NO_REFLECT,
    "svg/resvg/paint-servers/radialGradient/spreadMethod=repeat.svg": NO_REPEAT,
    "svg/custom/masking/mask/mask-and-image-with-transparency.svg": "bug. we currently override the soft mask that"
                                                                    "is in place in the content stream by creating an"
                                                                    "SMask entry in the Image XObject for transparency",
    "svg/resvg/masking/mask/recursive-on-child.svg": "bug. the soft mask that will be created as part of the gradient"
                                                     "will override the soft mask currently in place when rendering",
    "svg/resvg/painting/stroke-linecap/zero-length-path-with-round.svg": "need to check how Chrome does it",
    "svg/resvg/painting/stroke-linecap/zero-length-path-with-square.svg": "need to check how Firefox does it",

    "svg/resvg/structure/image/embedded-svg-with-text.svg": "bug. need to rewrite logic for nested SVG images.",
}


def main():
    test_string = f"// This file was auto-generated by `{Path(__file__).name}`, do not edit manually.\n\n"
    test_string += "#![allow(non_snake_case)]\n\n"
    test_string += "#[allow(unused_imports)]\nuse std::path::PathBuf;\nuse crate::{run_test_impl, convert_svg};\nuse svg2pdf::Options;\n"

    test_string += """
#[allow(dead_code)]
pub fn get_svg_path(test_name: &str) -> PathBuf {
    PathBuf::from("svg").join(String::from(test_name) + ".svg")
}

#[allow(dead_code)]
pub fn run_test(test_name: &str) -> i32 {
    let svg_path = get_svg_path(test_name);
    let (pdf, actual_image) = convert_svg(&svg_path, Options::default());
    run_test_impl(pdf, actual_image, test_name)
}\n
"""

    for p in SVG_DIR.rglob("*"):
        if p.is_file() and p.suffix == ".svg":
            test_file = TestFile(p)

            function_name = (
                str(test_file.test_name())
                .replace("/", "_")
                .replace("-", "_")
                .replace("=", "_")
                .replace(".", "_")
                .replace("#", "")
            )

            if str(test_file.svg_path()) in IGNORE_TESTS:
                test_string += f"// {IGNORE_TESTS[str(test_file.svg_path())]}\n"
                test_string += "#[ignore] "

            test_string += "#[test] "

            test_string += f'fn {function_name}() {{assert_eq!(run_test("{test_file.test_name()}"), 0)}}\n'

    with open(Path(OUT_PATH), "w") as file:
        file.write(test_string)


if __name__ == "__main__":
    main()
