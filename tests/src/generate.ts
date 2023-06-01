import {glob} from "glob";
import path from "path";
import cliProgress from "cli-progress"
import {
    generateAndWritePDF,
    generateAndWritePNG, generatePDFPath, generateReferencePath, generateSVGPath,
    referencesFolderPath, replaceExtension,
    svgFolderPath, SKIPPED_FILES, clearPDFs
} from "./util";

// Generates reference images. If the subdirectory is specified, only the reference images of
// the svgs in that subdirectory will be generated. If update is true, only existing reference
// images will be updated, but no new ones will be created.
async function generateReferenceImages(subdirectory: string = "", update: boolean = false) {
    let existingReferencesForSVGs = (await glob(path.join(subdirectory, '**/*.png'), {cwd: referencesFolderPath}))
        .map(imagePath => replaceExtension(imagePath, "svg"));

    let svgFilePaths = (await glob(path.join(subdirectory, '**/*.svg'), {cwd: svgFolderPath})).filter(svgPath => {
        if (update) {
            return existingReferencesForSVGs.includes(svgPath);
        } else {
            return true;
        }
    }).filter(el => !SKIPPED_FILES.includes(el));

    console.log("Starting with the generation...");
    let svgParentDirectory = path.join(svgFolderPath, subdirectory);
    console.log("Target directory: " + path.resolve(svgParentDirectory));
    console.log("Creating " + svgFilePaths.length + " images in total.");

    const progressBar = new cliProgress.SingleBar({}, cliProgress.Presets.shades_classic);
    progressBar.start(svgFilePaths.length, 0);

    for (let i = 0; i < svgFilePaths.length; i++) {
        progressBar.update(i);
        let svgFilePath = svgFilePaths[i];
        let svgFullPath = generateSVGPath(svgFilePath);
        let pdfFullPath = generatePDFPath(svgFilePath);

        await generateAndWritePDF(svgFullPath, pdfFullPath);

        let referenceImageFullPath = generateReferencePath(svgFilePath);

        await generateAndWritePNG(pdfFullPath, referenceImageFullPath);
    }

    progressBar.stop();

    console.log("Reference images were created successfully!");
    console.log("Cleaning up...");
    await clearPDFs();
}

(async function () {
    var argv = require('minimist')(process.argv.slice(2));
    let subdirectory: string = argv["subdirectory"] || "";
    let update: boolean = argv["update"] || false;

    await generateReferenceImages(subdirectory, update);
})();