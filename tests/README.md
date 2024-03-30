# Introduction

PDF is a very complex format, and because of this, writing your own PDF files programmatically
can be hard. Not only that, but because of the many PDF viewers out there with different
implementations, if you do accidentally create an invalid PDF, it is possible that it renders
fine on one PDF viewer but breaks using a different one. Because of this, it is important
that there exists some kind of automatic test suite that checks whether changes to this library
have unintended consequences for the output using visual regression tests.

Adobe Acrobat would arguably be the most relevant PDF viewer and thus should be the primary
targets for testing the PDF outputs of this library, however, since the Adobe PDF renderer is
not available as a library that can be linked to a program, we can't really test this. Because
of this, the next best option was chosen: `pdfium`, which is the library that powers the
PDF viewer in Chrome. In order to run the tests, you will have to get a copy of the [pdfium library](https://github.com/bblanchon/pdfium-binaries/releases)
for your operating system and put it into the "pdfium" folder. For full reproducibility,
use [this version](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F5880),
which is the one the CI uses.

In the `svg` folder, you can find all test files that are part of the test suite. They mostly
comprise the files of the [resvg test suite](https://github.com/RazrFalcon/resvg/tree/master/crates/resvg/tests/tests),
which is very comprehensive (1500+ files) and covers a big part of the SVG spec.
You can find the tests in `svg/resvg`. In addition to that, a couple of custom tests were added
that cover certain other edge cases and some integration tests specific to `svg2pdf1 . You can find them in
`svg/custom`. In the `ref` folder, you can find the corresponding reference images.

# Tests

We use a Python script to generate the `integration.rs` file, which tests all of the svg files
that are part of the test suites. SVG files which don't have a corresponding reference image
will be skipped. To regenerate this file, you can simply run `./scripts/gen-tests.py` and
it should work out of the box. 

Note: The aim of the test cases it not necessarily to check whether the SVGs are rendered
correctly, but rather whether our SVGs output match the one from resvg. In most cases, if
it matches resvg it is also correct, because resvg is pretty good at rendering SVGs in general.
However, a couple of features are not implemented (e.g. `enableBackground` in filters), thus
they are not rendered correctly, but we consider the test case as passing anyway, because
it matches the output of resvg.

To run the tests, you can then just invoke `cargo test --release` (make sure to run it
in release mode, otherwise it will be very slow).

A test case fails if the rendered PDF doesn't pixel-match the reference image. In this
case, a new folder `diff` will be generated that contains a diff image (the left image
is the expected image, the middle one the pixel difference and the right one the actual
image). If some parts of the core logic of the program have been changed, it is possible
that the images rendered by pdfium actually looks _pretty much_ the same but only differs
in a few sub-pixel ranges. You will notice this if you look at the diff image. If this is the
case for all failed tests, you can just run the command `./scripts/gen-tests.py --replace` and
then rerun the test suite, in which case all reference images of the failed tests will 
be overridden with the new ones. Don't forget to run the script again without the `--replace`
flag, as by default reference images shouldn't be overridden upon test failure.

In the `scripts` folder you can find some other scripts, but those are generally
not relevant for you and you can ignore them.