#!/usr/bin/env python3
import argparse
import shutil

from common import SVG_DIR, ROOT, TestFile
from pathlib import Path

OUT_PATH = ROOT / "src" / "test.rs"

NO_RELATIVE_PATHS = "no relative paths supported"
INVESTIGATE = "need to investigate"
NO_REFLECT = "spreadMethod reflect not supported"
NO_REPEAT = "spreadMethod repeat not supported"
NO_SUPPORT = "not supported in PDF"
NO_FONT = "font is not part of test suite yet"

IGNORE_TESTS = {
    "svg/resvg/filters/feMorphology/huge-radius.svg": "will timeout CI",
    "svg/resvg/filters/filter/huge-region.svg": "will sigkill",
    "svg/resvg/structure/svg/negative-size.svg": "invalid size",
    "svg/resvg/structure/svg/no-size.svg": "invalid size",
    "svg/resvg/structure/svg/zero-size.svg": "invalid size",
    "svg/resvg/structure/svg/not-UTF-8-encoding.svg": "invalid encoding",
    "svg/resvg/filters/feImage/simple-case.svg": NO_RELATIVE_PATHS,
    "svg/resvg/filters/fePointLight/primitiveUnits=objectBoundingBox.svg": INVESTIGATE,
    "svg/resvg/filters/filter/with-mask-on-parent.svg": INVESTIGATE,
    "svg/resvg/masking/mask/recursive-on-child.svg": INVESTIGATE,
    "svg/resvg/paint-servers/linearGradient/attributes-via-xlink-href-complex-order.svg": NO_REFLECT,
    "svg/resvg/paint-servers/linearGradient/attributes-via-xlink-href-from-radialGradient.svg": NO_REFLECT,
    "svg/resvg/paint-servers/linearGradient/spreadMethod=reflect.svg": NO_REFLECT,
    "svg/resvg/paint-servers/linearGradient/spreadMethod=repeat.svg": NO_REPEAT,
    "svg/resvg/paint-servers/pattern/nested-objectBoundingBox.svg": INVESTIGATE,
    "svg/resvg/paint-servers/radialGradient/attributes-via-xlink-href-complex-order.svg":NO_REFLECT,
    "svg/resvg/paint-servers/radialGradient/attributes-via-xlink-href-from-linearGradient.svg": NO_REFLECT,
    "svg/resvg/paint-servers/radialGradient/fr=0.2.svg": INVESTIGATE,
    "svg/resvg/paint-servers/radialGradient/fr=0.5.svg": INVESTIGATE,
    "svg/resvg/paint-servers/radialGradient/fr=0.7.svg": INVESTIGATE,
    "svg/resvg/paint-servers/radialGradient/fr=-1.svg": INVESTIGATE,
    "svg/resvg/paint-servers/radialGradient/invalid-gradientUnits.svg": INVESTIGATE,
    "svg/resvg/paint-servers/radialGradient/spreadMethod=reflect.svg": NO_REFLECT,
    "svg/resvg/paint-servers/radialGradient/spreadMethod=repeat.svg": NO_REPEAT,
    "svg/resvg/painting/marker/with-an-image-child.svg": NO_RELATIVE_PATHS,
    "svg/resvg/painting/mix-blend-mode/color-dodge.svg": "pdfium bug",
    "svg/resvg/painting/stroke-dasharray/m-0.svg": INVESTIGATE,
    "svg/resvg/painting/stroke-linecap/zero-lengt-path-with-round.svg": NO_SUPPORT,
    "svg/resvg/painting/stroke-linecap/zero-lengt-path-with-square.svg": NO_SUPPORT,
    "svg/resvg/painting/stroke-linejoin/miter-clip.svg": NO_SUPPORT,
    "svg/resvg/structure/image/external-gif.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-jpeg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-png.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-svg.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-svg-with-transform.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/external-svgz.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/float-size.svg": NO_RELATIVE_PATHS,
    "svg/resvg/structure/image/image-with-float-size-scaling.svg": INVESTIGATE,
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
    "svg/resvg/structure/svg/funcIRI-parsing.svg": INVESTIGATE,
    "svg/resvg/structure/svg/funcIRI-with-invalid-characters.svg": INVESTIGATE,
    "svg/resvg/text/alignment-baseline/hanging-on-vertical.svg": NO_FONT,
    "svg/resvg/text/direction/rtl.svg": NO_FONT,
    "svg/resvg/text/direction/rtl-with-vertical-writing-mode.svg": NO_FONT,
    "svg/resvg/text/dominant-baseline/hanging.svg": NO_FONT,
    "svg/resvg/text/dominant-baseline/use-script.svg": NO_FONT,
    "svg/resvg/text/font/simple-case.svg": NO_FONT,
    "svg/resvg/text/font-family/bold-sans-serif.svg": NO_FONT,
    "svg/resvg/text/font-family/cursive.svg": NO_FONT,
    "svg/resvg/text/font-family/fallback-1.svg": NO_FONT,
    "svg/resvg/text/font-family/fantasy.svg": NO_FONT,
    "svg/resvg/text/font-family/monospace.svg": NO_FONT,
    "svg/resvg/text/font-family/sans-serif.svg": NO_FONT,
    "svg/resvg/text/font-family/serif.svg": NO_FONT,
    "svg/resvg/text/font-family/source-sans-pro.svg": NO_FONT,
    "svg/resvg/text/font-kerning/arabic-script.svg": NO_FONT,
    "svg/resvg/text/font-weight/bolder-with-clamping.svg": INVESTIGATE,
    "svg/resvg/text/font-weight/lighter-with-clamping.svg": INVESTIGATE,
    "svg/resvg/text/font-weight/lighter-without-parent.svg": INVESTIGATE,
    "svg/resvg/text/letter-spacing/mixed-scripts.svg": INVESTIGATE,
    "svg/resvg/text/letter-spacing/non-ASCII-character.svg": NO_FONT,
    "svg/resvg/text/letter-spacing/on-Arabic.svg": NO_FONT,
    "svg/resvg/text/text/bidi-reordering.svg": NO_FONT,
    "svg/resvg/text/text/compound-emojis.svg": INVESTIGATE,
    "svg/resvg/text/text/compound-emojis-and-coordinates-list.svg": INVESTIGATE,
    "svg/resvg/text/text/emojis.svg": INVESTIGATE,
    "svg/resvg/text/text/fill-rule=evenodd.svg": NO_FONT,
    "svg/resvg/text/text/rotate-on-Arabic.svg": NO_FONT,
    "svg/resvg/text/text/x-and-y-with-multiple-values-and-arabic-text.svg": NO_FONT,
    "svg/resvg/text/text/xml-lang=ja.svg": NO_FONT,
    "svg/resvg/text/text-anchor/on-tspan-with-arabic.svg": NO_FONT,
    "svg/resvg/text/textLength/arabic.svg": NO_FONT,
    "svg/resvg/text/textLength/arabic-with-lengthAdjust.svg": NO_FONT,
    "svg/resvg/text/textPath/complex.svg": NO_FONT,
    "svg/resvg/text/textPath/writing-mode=tb.svg": NO_FONT,
    "svg/resvg/text/tspan/bidi-reordering.svg": NO_FONT,
    "svg/resvg/text/unicode-bidi/bidi-override.svg": NO_FONT,
    "svg/resvg/text/writing-mode/arabic-with-rl.svg": NO_FONT,
    "svg/resvg/text/writing-mode/japanese-with-tb.svg": NO_FONT,
    "svg/resvg/text/writing-mode/mixed-languages-with-tb.svg": NO_FONT,
    "svg/resvg/text/writing-mode/mixed-languages-with-tb-and-underline.svg": NO_FONT,
    "svg/resvg/text/writing-mode/tb-and-punctuation.svg": NO_FONT,
    "svg/resvg/text/writing-mode/tb-with-rotate.svg": NO_FONT,
    "svg/resvg/text/writing-mode/tb-with-rotate-and-underline.svg": NO_FONT,
}

def main():
    parser = argparse.ArgumentParser(
        prog="gen-tests", description="Generate the test files for svg2pdf"
    )

    parser.add_argument("-r", "--replace", action="store_true")

    args = parser.parse_args()

    test_string = f"// This file was auto-generated by `{Path(__file__).name}`, do not edit manually.\n\n"
    test_string += "#![allow(non_snake_case)]\n\n"
    test_string += "#[allow(unused_imports)]\nuse crate::run_test;\n\n"

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
            elif not test_file.has_ref():
                pass
                resvg_path = str(test_file.ref_path())
                resvg_path = resvg_path.replace("ref/resvg/", "")

                resvg_root = Path("../../resvg/crates/resvg/tests/tests")
                load_path = (resvg_root / resvg_path).resolve()
                store_path = test_file.ref_path().resolve().with_stem(load_path.stem + "-1").with_suffix(".png")
                print(f"{load_path}, {store_path}")
                store_path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(load_path, store_path)
                # test_string += f"// unknown reason\n"
                # test_string += "#[ignore] "

            test_string += "#[test] "

            replace = "true" if args.replace else "false"

            test_string += f'fn {function_name}() {{assert_eq!(run_test("{test_file.svg_path()}", "{test_file.ref_path()}", "{test_file.diff_path()}", {replace}), 0)}}\n'

    with open(Path(OUT_PATH), "w") as file:
        file.write(test_string)


if __name__ == "__main__":
    main()
