#!/usr/bin/env python3

from common import SVG_DIR, ROOT, TestFile
from pathlib import Path

OUT_PATH = ROOT / "src" / "render.rs"


def main():
    test_string = "#![allow(non_snake_case)]\n\n"
    test_string += "#[allow(unused_imports)]\nuse crate::render;\n\n"

    for p in SVG_DIR.rglob("*"):
        if p.is_file() and p.suffix == ".svg":
            test_file = TestFile(p)

            function_name = (
                str(test_file.svg_path.relative_to(SVG_DIR).with_suffix(""))
                .replace("/", "_")
                .replace("-", "_")
                .replace("=", "_")
                .replace(".", "_")
                .replace("#", "")
            )
            svg_path = test_file.svg_path
            ref_path = test_file.ref_path

            if not test_file.has_ref():
                test_string += "#[ignore] "

            test_string += "#[test] "

            test_string += f'fn {function_name}() {{assert_eq!(render("{svg_path.relative_to(ROOT)}", "{ref_path.relative_to(ROOT)}"), 0)}}\n'

    with open(Path(OUT_PATH), "w") as file:
        file.write(test_string)


if __name__ == "__main__":
    main()
