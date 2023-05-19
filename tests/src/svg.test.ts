import {glob} from "glob";
import {
    buildBinary,
    generateAndWritePDF,
    generatePNG,
    pdfPath,
    referencesPath,
    replaceExtension,
    svgPath
} from "./util";
import {assert} from "chai";
import path from "path";
import {readFileSync} from "fs";
import looksSame from "looks-same";
//
// let svgFilePaths = await glob('**/*.svg', {cwd: svgPath});
// let referenceImageFilePaths = await glob('**/*.png', {cwd: referencesPath});

const getPaths = async () => {
    let svgFilePaths = await glob('**/*.svg', {cwd: svgPath});
    let referenceImageFilePaths = await glob('**/*.png', {cwd: referencesPath});
    return {svgFilePaths, referenceImageFilePaths};
};

(async function () {
    let {svgFilePaths, referenceImageFilePaths} = await getPaths();
    console.log("Building pdf2svg...");
    await buildBinary();
    console.log("Build complete.")

    svgFilePaths.forEach(svgFilePath => {
        it('svg file ' + svgFilePath, async function() {
            // We skip svg files where there are no existing reference images
            let correspondingImagePath = replaceExtension(svgFilePath, "png");
            if (!referenceImageFilePaths.includes(correspondingImagePath)) {
                this.skip();
            }

            let svgInputPath = path.join(svgPath, svgFilePath);
            let pdfOutputPath = path.join(pdfPath, replaceExtension(svgFilePath, "pdf"));
            await generateAndWritePDF(svgInputPath, pdfOutputPath);

            let pdfInputPath = pdfOutputPath;
            let resultingImage = Buffer.from(await generatePNG(pdfInputPath));
            let referenceImage = await readFileSync(path.join(referencesPath, replaceExtension(svgFilePath, "png")))

            const {equal} = await looksSame(resultingImage, referenceImage, {strict: true});
            assert(equal);
        })
    })

    run();
})();