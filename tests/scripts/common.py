from pathlib import Path

ROOT = Path(__file__).parent.parent
SVG_DIR = ROOT / "svg"
REF_DIR = ROOT / "ref"
DIFF_DIR = ROOT / "diff"


class TestFile:
    def __init__(self, path: Path):
        self.__svg_path = path

        ref_path = path.with_suffix(".png")
        self.__ref_path = Path(REF_DIR / ref_path.relative_to(SVG_DIR))

        diff_path = path.with_suffix(".png")
        self.__diff_path = Path(DIFF_DIR / diff_path.relative_to(SVG_DIR))

    def test_name(self) -> Path:
        return self.__svg_path.relative_to(SVG_DIR).with_suffix("")

    def svg_path(self, relative: bool = True) -> Path:
        if relative:
            return self.__svg_path.relative_to(ROOT)
        else:
            return self.__svg_path

    def ref_path(self, relative: bool = True) -> Path:
        if relative:
            return self.__ref_path.relative_to(ROOT)
        else:
            return self.__ref_path

    def diff_path(self, relative: bool = True) -> Path:
        if relative:
            return self.__diff_path.relative_to(ROOT)
        else:
            return self.__diff_path

    def has_ref(self) -> bool:
        return self.ref_path(relative=False).is_file()
