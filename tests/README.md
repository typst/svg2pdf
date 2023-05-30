# svg2pdf Test Suite

This is the test suite for the svg2pdf library. 

## Motivation
In the past, in order to make sure that changes in the svg2pdf library
didn't have any unintended consequences for certain svgs, we had a number
of test svgs that we used to manually test whether the library still works.
However, there were two big problems with that approach:

- There were only around 20 test cases and there were not very representative,
meaning that they didn't provide a good coverage.
- We had to manually check that the output PDF of every SVG looked the same,
which gets very exhausting after some time.

One of the main difficulties is that if we want to implement automatic tests,
we somehow need to make sure that the actual visual output of the PDFs doesn't
change instead of just checking whether the PDF file itself change. The best way
of doing that would be to render the PDFs into images and then compare the images
with each other. However, at the point when this test suite was written, there wasn't
any Rust library that could reliably do that, which is why it was decided to use
a Node.js based setup instead.

The current setup is now as follows: We have a number of test svgs which reprsent 
the test cases. These svgs are then turned into a PDF using svg2pdf. Finally, we
turn the PDFs into png images using the PDF.js library (used in Firefox). The resulting
images are then visually compared against reference images. There are two important consequences
resulting from that setup:

- We have to rely on PDF.js to rasterize images correctly.
- When adding new reference images, they need to be checked manually to ensure that they
look how they actually should look. It was not possible to use the reference images that were part
of the resvg test suite (see below) because the rasterized outputs always tended to be slightly different,
so they needed to be reviewed manually.

## Test cases
You can find all svg test cases in the `svgs` folder. You will find two different folders
there: One is called `resvg` and the other one is called `custom`. `resvg` contains test cases
that are part of the [resvg-test-suite](https://github.com/RazrFalcon/resvg-test-suite), which is
a svg test suite consisting of around 1.600 images that were used to test the `resvg` library. Currently,
the pdf2svg test suite contains all test cases except for the ones in the `filters` and `text` folder.
The `filters` ones were not included because pdf2svg currently doesn't support filters and the `text`
ones weren't added due to font-rendering issues with GitHub Actions (it seems like texts are rendered
a bit differently on the CI meaning that test cases containing text always failed. This 
is probably fixable by identifying the cause for this, but for now text tests are not part of
this test suite).

The tests in `custom` are specific tests that were created just for the pdf2svg library, for example
in response to bug fixes. So if you are working on a bug fix or some specific feature that isn't covered
by the resvg-test-suite, you should add your test in that folder. There are no specific requirements for
what the test should look like, you should only make sure that it is as granular as possible (i.e.
it tests one specific feature/bug) and not too big in size (most test cases are around 200x200 pixels).

In the `references` folder, you can find the corresponding reference images to the svg files.
However, not all svg files contain corresponding reference images. There are two reasons
why this might be the case:

- **Failing test**: In most cases, some reference images have been removed because svg2pdf doesn't
render them correctly yet. A good example for this are the test files in `structure/image`, where
pretty much all test cases failed because image rendering isn't implemented correctly yet. However,
since we want the test suite to pass anyway until these issues is fixed (since the current purpose of 
the test suite is not to ensure that all test cases work correctly but instead to make sure that
test cases that worked previously don't stop working), they have been removed from the reference images
and thus the tests will be skipped when running `npm test`.
- **Invalid tests**: In some cases, it didn't make sense to include a test case 
from the `resvg-test-suite` in this test suite. However, instead of just deleting them
from the repository, it was decided to instead just ignore them at runtime (see the `util.ts` file).

## Scripts

There are only two scripts that are important

### Testing
If you made some changes to the svg2pdf library and you just want to make sure that all
existing test cases still work, you just need to
1. Build pdf2svg with `cargo build --release --features cli`. The test library will call the executable
by doing `../target/release/svg2pdf` so it's important that you build it yourself beforehand.
2. Start the tests by running `npm test`. As long as you didn't make any breaking changes, there should
only be passing and pending tests in the end.

If a test fails, there will be a diff image created in the `diffs` folder which can be used
to analyze how the resulting image differes from the reference image.

### Generating
If you made some other changes that require you to generate new reference images or 
regenerate existing ones (because the old ones were wrong or because you implemented 
a feature that hasn't been tested yet), you need to do `npm run generate`. There are a
couple of points to keep in mind:

- If you just do `npm run generate`, the script will generate reference images for all
existing ones in the `svgs` folder, except for the ones that have explicitly been excluded
in `util.ts`. **This also includes reference images that have been deleted because they are
not rendered correctly yet!** So it is unlikely that you want to execute this command as is.
- If you specify the update flag (`npm run generate -- --update`), only existing reference 
images will be regenerated
- You can also specify a specific subdirectory using the subdirectory argument. For example
if you believe that you fixed the rendering of images you could then run (
`npm run generate -- --subdirectory resvg/structure/image`) and then you can manually check
whether all reference images look correct now. The ones that look correct you can leave as 
they are and include them in the git repository, the ones that are not correct can be deleted 
again so that they are skipped when running the tests.

## Future Improvements
The test suite is not perfect in any regard, so ideas for improvement are welcome. Some
specific points:
- **oxipng**: Initially, the plan was to compress all reference images with oxipng to make them smaller. However,
when trying that we ran into issues with loading some images using the `looks-same` crate. It would be
good if the root cause of this could be investigated and maybe fixed. But for now, the reference images take up
around 5MB which isn't too bad.
- **text svgs**: We ran into some issues getting test cases that contain text to work properly using GitHub
actions, probably because there are some differences in how the text is rendered there (as mentioned above).
It would be good if this could be investigated and fixed so that we can include text tests as well.
- **calling the pdf2svg library**: Currently, the pdf2svg library will be called using the shell, and the PDF
files will be stored in a temporary directory. Maybe it is possible to somehow create Node bindings for pdf2svg
so that the PDF buffer can be directly stored in a variable instead of having to store it as a file first.