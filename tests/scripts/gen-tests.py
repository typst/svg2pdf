#!/usr/bin/env python3
import argparse

from common import SVG_DIR, ROOT, TestFile
from pathlib import Path

OUT_PATH = ROOT / "src" / "render.rs"


def main():
    parser = argparse.ArgumentParser(
        prog="gen-tests", description="Generate the test files for svg2pdf"
    )

    parser.add_argument("-r", "--replace", action="store_true")

    args = parser.parse_args()

    test_string = "#![allow(non_snake_case)]\n\n"
    test_string += "#[allow(unused_imports)]\nuse crate::render;\n\n"

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

            if not test_file.has_ref():
                test_string += "#[ignore] "

            test_string += "#[test] "

            replace = "true" if args.replace else "false"

            test_string += f'fn {function_name}() {{assert_eq!(render("{test_file.svg_path()}", "{test_file.ref_path()}", "{test_file.diff_path()}", {replace}), 0)}}\n'

    with open(Path(OUT_PATH), "w") as file:
        file.write(test_string)


if __name__ == "__main__":
    main()
