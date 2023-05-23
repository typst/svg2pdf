import path from "path";
import * as pdf2img from "pdf-img-convert";
import {existsSync, mkdirSync, writeFile} from "fs";
import {promisify} from "util";
import fs from "fs";

const svgFolderPath = "svgs";
const referencesFolderPath = "references";
const pdfsFolderPath = "pdfs";
const diffsFolderPath = "diffs";

const pdf2svgBinaryPath = path.join("..", "target", "release", "svg2pdf");
const exec = promisify(require('child_process').exec);

const SKIPPED_FILES = [
    'structure/svg/zero-size.svg',
    'structure/svg/not-UTF-8-encoding.svg',
    'structure/svg/negative-size.svg',
    'shapes/rect/vw-and-vh-values.svg',
    'shapes/rect/vmin-and-vmax-values.svg',
    'shapes/rect/vi-and-vb-values.svg',
    'shapes/rect/rlh-values.svg',
    'shapes/rect/rem-values.svg',
    'shapes/rect/q-values.svg',
    'shapes/rect/lh-values.svg',
    'shapes/rect/ic-values.svg',
    'shapes/rect/ch-values.svg',
    'shapes/rect/cap-values.svg',
]

// Builds pdf2svg
async function buildBinary() {
    try {
        await exec("cargo build --release --features cli");
    } catch (e: any) {
        throw new Error("unable to build pdf2svg: " + e.message);
    }
}

// Converts the svg from the input path to a pdf in the output path
async function generateAndWritePDF(inputPath: string, outputPath: string) {
    let outputFolderPath = path.dirname(outputPath);
    let command = pdf2svgBinaryPath + ' ' + inputPath + ' ' + outputPath;

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    try {
        await exec(command);
    } catch (e: any) {
        throw new Error("error while generating the pdf: " + e.message);
    }
}

// Converts a pdf from the input path to a png and returns it as a buffer
async function generatePNG(inputFilePath: string) {
    let pdfImage = await pdf2img.convert(inputFilePath, {scale: 2.5, page_numbers: [1]});

    if (pdfImage.length !== 1) {
        throw new Error("expected pdf of length 1, found pdf of length " + pdfImage.length);
    }

    return pdfImage[0];
}

// Converts a pdf from the input path to a png and writes it into the output path
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

async function writeDiffImage(diffImage: Buffer, outputPath: string) {
    let outputFolderPath = path.dirname(outputPath);

    if (!existsSync(outputFolderPath)) {
        mkdirSync(outputFolderPath, {recursive: true});
    }

    await writeFile(outputPath, diffImage, function (error) {
        if (error) {
            throw new Error("unable to write diff image to file system: " + error)
        }
    });
}

// Optimizes an image using oxipng
async function optimize(filePath: string) {
    try {
        await exec("oxipng " + filePath);
    }   catch (e: any) {
        throw new Error("unable to optimize image: " + e.message);
    }
}

function generateFullPath(parentFolder: string, filePath: string, extension: string) {
    return path.join(parentFolder, path.dirname(filePath),
        path.basename(filePath, path.extname(filePath)) + "." + extension);
}

// Takes a path of something like 'shapes/lines/no-coordinates.svg' and turns it into
// 'svgs/shapes/lines/no-coordinates.svg'
function generateSVGPath(filePath: string) {
    return generateFullPath(svgFolderPath, filePath, "svg");
}

// Takes a path of something like 'shapes/lines/no-coordinates.svg' and turns it into
// 'references/shapes/lines/no-coordinates.png'
function generateReferencePath(filePath: string) {
    return generateFullPath(referencesFolderPath, filePath, "png");
}

// Takes a path of something like 'shapes/lines/no-coordinates.svg' and turns it into
// 'pdfs/shapes/lines/no-coordinates.pdf'
function generatePDFPath(filePath: string) {
    return generateFullPath(pdfsFolderPath, filePath, "pdf");
}

// Takes a path of something like 'shapes/lines/no-coordinates.svg' and turns it into
// 'diffs/shapes/lines/no-coordinates.png'
function generateDiffsPath(filePath: string) {
    return generateFullPath(diffsFolderPath, filePath, "png");
}

function replaceExtension(replacePath: string, extension: string) {
    return path.join(path.dirname(replacePath),
    path.basename(replacePath, path.extname(replacePath)) + "." + extension);
}

function clearPDFs() {
    if (existsSync(pdfsFolderPath)) {
        fs.rmSync(pdfsFolderPath, { recursive: true});
    }
}

function clearDiffs() {
    if (existsSync(diffsFolderPath)) {
        fs.rmSync(diffsFolderPath, { recursive: true});
    }
}

export {
    svgFolderPath, referencesFolderPath, pdfsFolderPath, pdf2svgBinaryPath, generateAndWritePNG,
    buildBinary, generateAndWritePDF, optimize, replaceExtension, generatePNG, generateSVGPath,
    generatePDFPath, generateReferencePath, generateDiffsPath, clearPDFs, clearDiffs, writeDiffImage,
    SKIPPED_FILES
}