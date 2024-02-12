#!/usr/bin/env python3

from pathlib import Path

from common import SVG_DIR, ROOT, TestFile

OUT_PATH = ROOT / "typst.typ"


def main():
    test_files = []
    for p in sorted(list(SVG_DIR.rglob("*"))):
        if p.is_file() and p.suffix == ".svg":
            test_file = TestFile(p)

            if test_file.has_ref():
                test_files.append(
                    f'  ("{test_file.svg_path()}", "{test_file.ref_path()}"),'
                )

    typst_string = f"""
#set page(width: 2000pt, height: 1200pt, margin: 0pt)

#let files = (
{chr(10).join(test_files)}
)

#for (svg-path, ref-path) in files {{
  box(grid(
    columns: (200pt, 200pt),
    column-gutter: 0pt,
    stack(
      dir: ttb,
      spacing: 3pt,
      align(center, text(
        size: 15pt,
        [Expected])
      ),
      image(ref-path, width: 200pt),
      text(size: 8pt, svg-path),
    ),
    stack(
      dir: ttb,
      spacing: 3pt,
      align(center, text(
        size: 15pt,
        [Actual])
      ),
      image(svg-path, width: 200pt),
      text(size: 8pt, svg-path),
    )
  ))
}}
"""

    with open(Path(OUT_PATH), "w") as file:
        file.write(typst_string)


if __name__ == "__main__":
    main()
