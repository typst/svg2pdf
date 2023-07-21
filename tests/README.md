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
for your operating system and put it into the "pdfium_lib" folder. For full reproducibility,
use [this version](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F5880),
which is the one the CI uses.

In the `svg` folder, you can find all test files that are part of the test suite. They mostly
comprise the files of the [resvg-test-suite](https://github.com/RazrFalcon/resvg-test-suite),
(filter tests have not been included yet since they are not implemented) which is a comprehensive suite of SVG files (1000+ files) that cover a big part of the SVG spec.
You can find the tests in `svg/resvg`. In addition to that, a couple of custom tests were added
that cover certain other edge cases and some integration tests. You can find them in
`svg/custom`. In the `ref` folder, you can find the corresponding reference images.

There are three binary targets in this crate: `test`, `generate` and `typst`.

# Test

The `test` target allows you to run the whole test suite to check whether the current
implementation of `svg2pdf` produces the same output as the ones before. You should run it
using the following command: `cargo run --release`, or alternatively `cargo run --release --bin test` (Make sure to run it in release mode,
otherwise it will be very slow!)

Once you run this command, it will go through each test case and print out the ones that were
skipped and the ones that were failed. If you want to see every file that has been tested,
you can pass the verbose flag to the command: `cargo run --release -- --verbose`.

- A test case will be skipped if there exists an SVG file that doesn't have a corresponding
reference image. Currently, this is the case for a few tests that either don't work correctly
yet (like for example `svg/resvg/paint-servers/stop/stops-with-equal-offset-5.svg`), are simply
not implemented yet (e.g. spread method of gradients and blend modes) or were skipped simply
because it wasn't deemed necessary to add them (for example text tests that test
Japanese/Arabic that would've required to add additional fonts to the repository).

- A test case fails if the rendered PDF doesn't pixel-match the reference image. In this
case, a new folder `diff` will be generated that contains a diff image (the left image
is the expected image, the middle one the pixel difference and the right one the actual
image). If some parts of the core logic of the program have been changed, it is possible
that the images rendered by pdfium actually looks _pretty much_ the same but only differs
in a few sub-pixel ranges. You will notice this if you look at the diff image. If this is the
case for all failed tests, you can just run the command `cargo run --release -- --replace`,
in which case all reference images of the failed tests will be overridden with the
new ones. Then you just need to check the new reference images into the repository. However,
if an image significantly differs from its reference image, you will have to investigate the
cause of that. Most likely some bug was introduced, but it's also possible that the reference
image itself is wrong (they have been cross-checked manually, but mistakes happen).

- If the new image does match the reference image, the test will pass.

# Generate

The `generate` target allows you to regenerate reference images.
You can run it using the command `cargo run --release --bin generate`. If you run this command you will notice that it is
relatively slow, especially for the custom integration SVGs which
are much bigger. The main culprit here is the optimization
of the reference images using `oxipng` though, the generation
using `svg2pdf` itself is pretty fast, which you can also notice
by the fact that the `test` command runs pretty quickly.

If you run the above command, the reference images _of all SVGs
that already have reference images_ will be regenerated. If you want to generate reference images for _all_ SVG images, you can
do so by passing the `--full` flag. However, you rarely should have
to do that.

In most cases, you will probably want to use this command to generate reference images for new test cases you added. In this
case, you can use the `--subset` flag, where you can pass a regex
that will be matched against all SVG files. For example, if you run
`cargo run --bin generate -- --subset "arabic"`, reference images for every test file where the path name contains "arabic" will
be generated.

If you want to generate the actual PDFs instead of images, you can pass the `--pdf` flag instead.

# Typst

The final target is `typst`. This library was initially written
for [Typst](https://github.com/typst/typst/), a typesetting engine
written in Rust. While the test suite is already a good indicator
of whether the program works or not, we wanted to have a way of
manually checking whether the SVGs look as expected when embedding
them with Typst. This also allows us to identify issues that are
specific to certain PDF rendering engines.

Once you run this command, a new Typst file will be generated
that embeds all test and reference images next to each other
so that you can compare them easily. You can then just render
this file using Typst and inspect the output in a PDF viewer
of your choice (keep in mind that you need to load the Noto
Sans fonts in Typst as well to get the same output).

Some observations that were made:
- **Google Chrome**: Everything looks the same as in the reference
images.
- **Adobe Acrobat**: Some colors seem to have a different "tone"
and patterns on stroke are rendered a bit differently, but
other than that everything looks the same.
- **Firefox**: Stop opacities don't seem to be displayed
correctly; patterns on stroke look a bit different. Otherwise,
the rest looks correct.
- **Safari**: Unfortunately, Safari has quite a few issues. Soft
masks aren't displayed properly at all (they work when opening
a PDF directly created by svg2pdf, but not once they are
embedded via Typst). In addition, transforms on patterns and gradients
are not applied properly. However, this most likely is an issue
with the PDF rendering engine of Safari itself, so there is
not much we can do. Most of the test cases still display fine.
- **muPDF**: Patterns on strokes look a bit different, otherwise
everything looks the same.
