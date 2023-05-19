import {glob} from 'glob';
import path from "path";
import * as pdf2img from 'pdf-img-convert'
import {existsSync, mkdirSync, writeFile} from "fs";
import {promisify} from "util";

const exec = promisify(require('child_process').exec);
const svgFilesPath = path.parse("files");
const pdfFilesPath = path.parse("pdfs");
const actualFilesPath = path.parse("actual");
const pdf2svgBinaryPath = path.join("..", "target", "release", "svg2pdf");

const SKIPPED_FILES = [
    'structure/svg/zero-size.svg',
    'structure/svg/not-UTF-8-encoding.svg',
    'structure/svg/negative-size.svg',
]

async function buildBinary(): Promise<void> {
    try {
        console.log("Building pdf2svg...");
        await exec("cargo build --release --features cli");
        console.log("pdf2svg was build successfully.")
    } catch (e) {
        throw new Error("Build of pdf2svg failed.")
    }
}

async function generatePDF(filename: string) {
    let inputPath = path.join(svgFilesPath.name, filename);
    let outputFolderPath = path.join(pdfFilesPath.name, path.dirname(filename));
    let outputPath = path.join(outputFolderPath, path.parse(path.basename(filename)).name + ".pdf");
    let command = pdf2svgBinaryPath + ' ' + inputPath + ' ' + outputPath;

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    await exec(command);
}

async function generatePNG(filename: string) {
    let inputPath = path.join(pdfFilesPath.name, path.dirname(filename),
        path.parse(path.basename(filename)).name + ".pdf");
    let pdfImage = await pdf2img.convert(inputPath, {scale: 2.5, page_numbers: [1]});

    let outputFolderPath = path.join(actualFilesPath.name, path.dirname(filename));
    let outputPath = path.join(outputFolderPath, path.parse(path.basename(filename)).name + ".png");

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    await writeFile(outputPath, pdfImage[0], function (error) {
        if (error) { console.error("Error: " + error); }
    });

    await exec("oxipng " + outputPath);

}

async function generate() {
    let svgFiles = await glob('**/*.svg', {cwd: svgFilesPath.name});
    svgFiles = svgFiles.filter(el => !SKIPPED_FILES.includes(el));

    for (let filename of svgFiles) {
        await generatePDF(filename);
        await generatePNG(filename);
    }
}


(async function () {

    try {
        await buildBinary();
        await generate();
    } catch (e: any) {
        console.error("Testing was unsuccessful. Error: " + e.message)
    }

    // const svgFiles = await glob('tests/**/*.svg');
    // console.log(svgFiles);

    // pdfArray = await pdf2img.convert('test.pdf', {scale: 2.5, page_numbers: [1]});
    // console.log("saving");
    // for (i = 0; i < pdfArray.length; i++){
    //     fs.writeFile("output"+i+".png", pdfArray[i], function (error) {
    //         if (error) { console.error("Error: " + error); }
    //     }); //writeFile
    // } // for
})();