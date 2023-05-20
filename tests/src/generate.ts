import {glob} from "glob";
import path from "path";
import cliProgress from "cli-progress"
import {
    buildBinary, clearPDFs,
    generateAndWritePDF,
    generateAndWritePNG, generatePDFPath, generateReferencePath, generateSVGPath,
    optimize, referencesFolderPath, replaceExtension,
    svgFolderPath
} from "./util";

async function generateReferenceImages(subdirectory: string = "", update: boolean = true) {
    // Allows us to regenerate only a subdirectory of files
    let existingReferencesForSVGs = (await glob(path.join(subdirectory, '**/*.png'), {cwd: referencesFolderPath}))
        .map(imagePath => replaceExtension(imagePath, "svg"));

    let svgFilePaths = (await glob(path.join(subdirectory, '**/*.svg'), {cwd: svgFolderPath})).filter(svgPath => {
        if (update) {
            return existingReferencesForSVGs.includes(svgPath);
        } else {
            return true;
        }
    });

    clearPDFs();

    console.log("Building svg2pdf...");
    await buildBinary();

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
        await optimize(referenceImageFullPath);
    }

    progressBar.stop();
    console.log("Reference images were created successfully!");

    clearPDFs();
}

(async function () {
    await generateReferenceImages();
})();