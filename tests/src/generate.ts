import {glob} from "glob";
import path from "path";
import cliProgress from "cli-progress"
import {
    buildBinary, clearPDFs,
    generateAndWritePDF,
    generateAndWritePNG, generatePDFPath, generateReferencePath, generateSVGPath,
    optimize,
    svgFolderPath
} from "./util";

async function generateReferenceImages(subdirectory: string = "") {
    // Allows us to regenerate only a subdirectory of files
    let svgParentDirectory = path.join(svgFolderPath, subdirectory);
    let svgFilePaths = (await glob('**/*.svg', {cwd: svgParentDirectory}));

    clearPDFs();

    console.log("Building svg2pdf...");
    await buildBinary();
    console.log("Starting with the generation...");
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