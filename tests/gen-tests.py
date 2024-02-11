#!/usr/bin/env python3

from pathlib import Path

SVG_DIR = "svg"
REF_DIR = "ref"


class TestFile:
    def __init__(self, path: Path):
        self.svg_path = path

        ref_path = path.with_suffix(".png")
        parts = list(ref_path.parts)
        parts[0] = REF_DIR
        self.ref_path = Path(*parts)


def main():

    test_string = "#![allow(non_snake_case)]\n\n"
    test_string += "use crate::render;\n\n"

    counter = 0

    for p in Path(SVG_DIR).rglob("*"):
        if p.is_file() and p.suffix == ".svg":
            test_file = TestFile(p)

            # counter += 1
            #
            # if counter == 100:
            #     break

            function_name = str(test_file.svg_path.with_suffix("")) \
                .replace("/", "_") \
                .replace("-", "_") \
                .replace("=", "_") \
                .replace(".", "_") \
                .replace("#", "")
            svg_path = test_file.svg_path
            ref_path = test_file.ref_path

            if not ref_path.is_file():
                test_string += "// "

            test_string += "#[test] "

            test_string += f"fn {function_name}() {{assert_eq!(render(\"{svg_path}\", \"{ref_path}\"), 0)}}\n"

    print(test_string)


if __name__ == '__main__':
    main()
