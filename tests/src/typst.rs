use std::fs;

use svg2pdf_tests::*;

fn main() {
    let existing_svg_references: Vec<TestFile> =
        (*REF_FILES).iter().map(|f| TestFile::new(f)).collect();

    let file_list = (*SVG_FILES)
        .iter()
        .map(|f| TestFile::new(f))
        .filter(|f| existing_svg_references.contains(f))
        .map(|f| {
            format!(
                "(\"{}\", \"{}\")",
                f.as_svg_path().to_str().unwrap(),
                f.as_references_path().to_str().unwrap()
            )
        })
        .collect::<Vec<String>>()
        .join(",\n");

    let whole_file = format!(
        "
#set page(width: 2000pt, height: 1200pt, margin: 0pt)

#let files = (
{}
).map(pair => box(
    grid(columns: (200pt, 200pt),
    column-gutter: 0pt,
    stack(
        dir: ttb,
        spacing: 3pt,
        align(center,
          text(
            size: 15pt,
            [Expected])
          ),
        image(pair.at(1), width: 200pt),
        text(size: 8pt, pair.at(0))
      ),
      stack(
        dir: ttb,
        spacing: 3pt,
        align(center,
          text(
            size: 15pt,
            [Actual])
          ),
        image(pair.at(0), width: 200pt),
        text(size: 8pt, pair.at(0))
      )
  )
  )
)


#for file in files {{
  file
}}
",
        file_list
    );

    fs::write("typst.typ", whole_file).unwrap();
}
