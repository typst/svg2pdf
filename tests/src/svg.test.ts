import {glob} from "glob";
import {
    buildBinary, clearDiffs, clearPDFs,
    generateAndWritePDF, generateDiffsPath, generatePDFPath,
    generatePNG, generateReferencePath, generateSVGPath,
    pdfsFolderPath,
    referencesFolderPath,
    replaceExtension,
    svgFolderPath, writeDiffImage
} from "./util";
import {assert} from "chai";
import path from "path";
import {readFileSync} from "fs";
import looksSame from "looks-same";
import {fail} from "assert";

const getPaths = async () => {
    let svgFilesPaths = await glob('**/*.svg', {cwd: svgFolderPath});
    let referenceImageFilesPaths = await glob('**/*.png', {cwd: referencesFolderPath});
    return {svgFilePaths: svgFilesPaths, referenceImageFilesPaths: referenceImageFilesPaths};
};

const prepare = async () => {
    clearPDFs();
    clearDiffs();
};

(async function () {
    await prepare();
    let {svgFilePaths, referenceImageFilesPaths} = await getPaths();
    console.log("Building pdf2svg...");
    await buildBinary();
    console.log("Build complete.")

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

            let resultingImage = Buffer.from(await generatePNG(pdfFullPath));
            let referenceImage = await readFileSync(referenceImageFullPath);

            const {equal} = await looksSame(resultingImage, referenceImage, {strict: true});
            if (!equal) {
                const diffImage = await looksSame.createDiff({
                    reference: referenceImage,
                    current: resultingImage,
                    highlightColor: '#ff00ff',
                    strict: true
                });

                await writeDiffImage(diffImage, generateDiffsPath(svgFilePath));

                fail("images don't match");
            }
        })
    })

    run();
})();