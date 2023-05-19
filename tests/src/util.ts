import path from "path";
import * as pdf2img from "pdf-img-convert";
import {existsSync, mkdirSync, writeFile} from "fs";
import {promisify} from "util";

const svgFilesPath = "svgs";
const referencesPath = "references";
const pdfFilesPath = "pdfs";
const actualFilesPath = "actual";
const pdf2svgBinaryPath = path.join("..", "target", "release", "svg2pdf");
const exec = promisify(require('child_process').exec);

const SKIPPED_FILES = [
    'structure/svg/zero-size.svg',
    'structure/svg/not-UTF-8-encoding.svg',
    'structure/svg/negative-size.svg',
]

async function buildBinary() {
    try {
        await exec("cargo build --release --features cli");
    } catch (e) {
        throw new Error("unable to build pdf2svg");
    }
}

async function generatePDF(inputFilePath: string, outputFilePath: string) {
    let outputFolderPath = path.dirname(outputFilePath);
    let command = pdf2svgBinaryPath + ' ' + inputFilePath + ' ' + outputFilePath;

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    try {
        await exec(command);
    } catch (e) {
        throw new Error("error while generating the pdf");
    }
}

async function generatePNG(inputFilePath: string, outputFilePath: string) {
    let pdfImage = await pdf2img.convert(inputFilePath, {scale: 2.5, page_numbers: [1]});

    if (pdfImage.length !== 1) {
        throw new Error("expected pdf of length 1, found pdf of length " + pdfImage.length);
    }

    let outputFolderPath = path.dirname(outputFilePath);

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    await writeFile(outputFilePath, pdfImage[0], function (error) {
        if (error) {
            throw new Error("unable to write image to file system")
        }
    });
}

async function optimize(filePath: string) {
    try {
        await exec("oxipng " + filePath);
    }   catch (e) {
        throw new Error("unable to optimize image");
    }
}

export {
    svgFilesPath, referencesPath, pdfFilesPath,
    actualFilesPath, pdf2svgBinaryPath, generatePNG, SKIPPED_FILES,
    buildBinary, generatePDF, optimize
}