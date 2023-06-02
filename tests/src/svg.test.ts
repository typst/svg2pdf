import {glob} from "glob";
import {
    clearDiffs, clearPDFs,
    generateAndWritePDF, generateDiffsPath, generatePDFPath,
    generatePNG, generateReferencePath, generateSVGPath,
    referencesFolderPath,
    replaceExtension, SKIPPED_FILES,
    svgFolderPath, writeDiffImage
} from "./util";
import path from "path";
import {readFileSync} from "fs";
import looksSame from "looks-same";
import {fail} from "assert";

var argv = require('minimist')(process.argv.slice(2));
let subdirectory: string = argv["subdirectory"] || "";

const getPaths = async () => {
    let svgFilesPaths = (await glob(path.join(subdirectory, '**/*.svg'), {cwd: svgFolderPath})).filter(el => !SKIPPED_FILES.includes(el));
    let referenceImageFilesPaths = await glob(path.join(subdirectory, '**/*.png'), {cwd: referencesFolderPath});
    return {svgFilePaths: svgFilesPaths, referenceImageFilesPaths: referenceImageFilesPaths};
};

const prepare = async () => {
    clearPDFs();
    clearDiffs();
};

(async function () {
    await prepare();
    let {svgFilePaths, referenceImageFilesPaths} = await getPaths();

    svgFilePaths.forEach(svgFilePath => {
        it('svg file ' + svgFilePath, async function() {

            // We skip svg files where there are no existing reference images
            let referenceImagePath = replaceExtension(svgFilePath, "png");
            if (!referenceImageFilesPaths.includes(referenceImagePath)) {
                this.skip();
            }
            let svgFullPath = generateSVGPath(svgFilePath);
            let referenceImageFullPath = generateReferencePath(svgFilePath);
            let pdfFullPath = generatePDFPath(svgFilePath);
            await generateAndWritePDF(svgFullPath, pdfFullPath);

            let actualImage = Buffer.from(await generatePNG(pdfFullPath));
            let referenceImage = await readFileSync(referenceImageFullPath);

            const {equal} = await looksSame(actualImage, referenceImage, {strict: true});
            if (!equal) {
                const diffImage = await looksSame.createDiff({
                    reference: referenceImage,
                    current: actualImage,
                    highlightColor: '#ff0000',
                    strict: true
                });

                await writeDiffImage(diffImage, actualImage, referenceImage, generateDiffsPath(svgFilePath));

                fail("images don't match");
            }
        })
    })

    run();
})();