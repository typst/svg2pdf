import {glob} from "glob";
import path from "path";
import cliProgress from "cli-progress"
import {
    buildBinary,
    generateAndWritePDF,
    generateAndWritePNG,
    optimize,
    pdfPath,
    referencesPath, replaceExtension,
    SKIPPED_FILES,
    svgPath
} from "./util";
import * as fs from "fs";

async function generateReferenceImages(subdirectory: string = "") {
    // Allows us to regenerate only a subdirectory of files
    let svgParentDirectory = path.join(svgPath, subdirectory);
    let pdfParentDirectory = path.join(pdfPath, subdirectory);
    let pngParentDirectory = path.join(referencesPath, subdirectory);
    let svgFilePaths = (await glob('**/*.svg', {cwd: svgParentDirectory}))
        .filter(el => !SKIPPED_FILES.includes(el));

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
        let svgInputPath = path.join(svgParentDirectory, svgFilePath);
        let pdfOutputPath = path.join(pdfParentDirectory, replaceExtension(svgFilePath, "pdf"));

        await generateAndWritePDF(svgInputPath, pdfOutputPath);

        let pdfInputPath = pdfOutputPath;
        let imageOutputPath = path.join(pngParentDirectory, replaceExtension(svgFilePath, "png"));

        await generateAndWritePNG(pdfInputPath, imageOutputPath);
        await optimize(imageOutputPath);
    }

    progressBar.stop();
    console.log("Reference images were created successfully!");

    fs.rmSync(pdfPath, { recursive: true});
}

(async function () {
    await generateReferenceImages();
})();