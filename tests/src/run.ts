import {glob} from 'glob';
import path from "path";
import {existsSync, mkdirSync} from "fs";
import {promisify} from "util";

const exec = promisify(require('child_process').exec);
const svgFilesPath = path.parse("files");
const pdfFilesPath = path.parse("pdfs");
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

async function buildPDFs() {
    let svgFiles = await glob('**/*.svg', {cwd: svgFilesPath.name});
    svgFiles = svgFiles.filter(el => !SKIPPED_FILES.includes(el));

    for (let filename of svgFiles) {
        let input = path.join(svgFilesPath.name, filename);
        let outputFolder = path.join(pdfFilesPath.name, path.dirname(filename));
        let output = path.join(outputFolder, path.parse(path.basename(filename)).name + ".pdf");
        let command = pdf2svgBinaryPath + ' ' + input + ' ' + output;

        if (!existsSync(outputFolder)) {
            mkdirSync(outputFolder, {recursive: true});
        }

        await exec(command);
    }
}


(async function () {

    try {
        await buildBinary();
        await buildPDFs();
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