from pathlib import Path

ROOT = Path(__file__).parent.parent
SVG_DIR = ROOT / "svg"
REF_DIR = ROOT / "ref"
DIFF_DIR = ROOT / "diff"


class TestFile:
    def __init__(self, path: Path):
        self.svg_path = path

        ref_path = path.with_suffix(".png")
        self.ref_path = Path(REF_DIR / ref_path.relative_to(SVG_DIR))

        diff_path = path.with_suffix(".png")
        self.diff_path = Path(DIFF_DIR / diff_path.relative_to(SVG_DIR))

    def has_ref(self) -> bool:
        return self.ref_path.is_file()
