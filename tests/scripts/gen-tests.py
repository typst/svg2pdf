#!/usr/bin/env python3

from pathlib import Path

ROOT = Path(__file__).parent.parent
SVG_DIR = ROOT / "svg"
REF_DIR = ROOT / "ref"

print(SVG_DIR.resolve())


class TestFile:
    def __init__(self, path: Path):
        self.svg_path = path

        ref_path = path.with_suffix(".png")
        parts = list(ref_path.parts)
        parts[0] = REF_DIR
        self.ref_path = Path(*parts)


OUT_PATH = (ROOT / "src" / "render.rs")


def main():

    test_string = "#![allow(non_snake_case)]\n\n"
    test_string += "use crate::render;\n\n"

    for p in SVG_DIR.rglob("*"):
        if p.is_file() and p.suffix == ".svg":
            test_file = TestFile(p)

            function_name = str(test_file.svg_path.relative_to(SVG_DIR).with_suffix("")) \
                .replace("/", "_") \
                .replace("-", "_") \
                .replace("=", "_") \
                .replace(".", "_") \
                .replace("#", "")
            svg_path = test_file.svg_path
            ref_path = test_file.ref_path

            if not ref_path.is_file():
                test_string += "#[ignore] "

            test_string += "#[test] "

            test_string += f"fn {function_name}() {{assert_eq!(render(\"{svg_path}\", \"{ref_path}\"), 0)}}\n"

    with open(Path(OUT_PATH), "w") as file:
        file.write(test_string)


if __name__ == '__main__':
    main()
