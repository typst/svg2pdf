import path from "path";
import * as pdf2img from "pdf-img-convert";
import {existsSync, mkdirSync, writeFile} from "fs";
import {promisify} from "util";

const svgPath = "svgs";
const referencesPath = "references";
const pdfPath = "pdfs";
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
    } catch (e: any) {
        throw new Error("unable to build pdf2svg: " + e.message);
    }
}

async function generateAndWritePDF(inputFilePath: string, outputFilePath: string) {
    let outputFolderPath = path.dirname(outputFilePath);
    let command = pdf2svgBinaryPath + ' ' + inputFilePath + ' ' + outputFilePath;

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    try {
        await exec(command);
    } catch (e: any) {
        throw new Error("error while generating the pdf: " + e.message);
    }
}

async function generatePNG(inputFilePath: string) {
    let pdfImage = await pdf2img.convert(inputFilePath, {scale: 2.5, page_numbers: [1]});

    if (pdfImage.length !== 1) {
        throw new Error("expected pdf of length 1, found pdf of length " + pdfImage.length);
    }

    return pdfImage[0];
}

async function generateAndWritePNG(inputFilePath: string, outputFilePath: string) {
    let pdfImage = await generatePNG(inputFilePath);

    let outputFolderPath = path.dirname(outputFilePath);

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    await writeFile(outputFilePath, pdfImage, function (error) {
        if (error) {
            throw new Error("unable to write image to file system: " + error)
        }
    });
}

async function optimize(filePath: string) {
    try {
        await exec("oxipng " + filePath);
    }   catch (e: any) {
        throw new Error("unable to optimize image: " + e.message);
    }
}

function replaceExtension(replacePath: string, extension: string) {
    return path.join(path.dirname(replacePath),
    path.basename(replacePath, path.extname(replacePath)) + "." + extension);
}

export {
    svgPath, referencesPath, pdfPath, pdf2svgBinaryPath, generateAndWritePNG, SKIPPED_FILES,
    buildBinary, generateAndWritePDF, optimize, replaceExtension, generatePNG
}