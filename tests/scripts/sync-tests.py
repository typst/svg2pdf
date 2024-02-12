#!/usr/bin/env python3
# TODO: Prettify script.

from pathlib import Path

RESVG_SVG_DIR = Path("../../../resvg/crates/resvg/tests/tests")
SVG2PDF_SVG_DIR = Path("../svg/resvg")
SVG2PDF_REF_DIR = Path("../ref/resvg")

resvg_svg_files = set([p.relative_to(RESVG_SVG_DIR) for p in RESVG_SVG_DIR.rglob("*") if p.suffix == ".svg"])
svg2pdf_svg_files = set([p.relative_to(SVG2PDF_SVG_DIR) for p in SVG2PDF_SVG_DIR.rglob("*")])
svg2pdf_ref_files = set([p.relative_to(SVG2PDF_REF_DIR) for p in SVG2PDF_REF_DIR.rglob("*")])


def sync_existing_tests():
    print("Sync tests...")
    for file in resvg_svg_files:
        absolute_path = RESVG_SVG_DIR / file
        assert absolute_path.is_file()

        if str(file) == "structure/svg/not-UTF-8-encoding.svg":
            continue

        with open(absolute_path, "r") as resvg_file:
            content = resvg_file.read()

            path = SVG2PDF_SVG_DIR / file
            if not path.parent.exists():
                path.parent.mkdir(parents=True, exist_ok=True)

            with open(path, "w+") as svg2pdf_file:
                svg2pdf_file.write(content)



def find_superfluous_tests():
    superfluous_tests = [str(p) for p in svg2pdf_svg_files - resvg_svg_files if p.suffix == ".svg"]
    if len(superfluous_tests) == 0:
        print("No superfluous tests found.")
    else:
        print(f"Found {len(superfluous_tests)} superfluous tests: {sorted(superfluous_tests)}")


def find_superfluous_ref_images():
    svg_paths = [p.with_suffix(".svg") for p in svg2pdf_ref_files if p.suffix == ".png"]
    superfluous_refs = set(svg_paths) - svg2pdf_svg_files

    if len(superfluous_refs) == 0:
        print("No superfluous refs found.")
    else:
        print(f"Found {len(superfluous_refs)} superfluous tests: {sorted(superfluous_refs)}")


sync_existing_tests()
find_superfluous_tests()
find_superfluous_ref_images()
