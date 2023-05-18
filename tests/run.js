const pdf2img = require('pdf-img-convert');
const fs = require("fs");
const util = require('util');
const path = require('path');
const {
    glob,
    globSync,
    globStream,
    globStreamSync,
    Glob,
} = require('glob');

const exec = util.promisify(require('child_process').exec);
const svgFilesPath = path.parse("files");
const pdfFilesPath = path.parse("pdfs");
const pdf2svgBinaryPath = path.join("..", "target", "release", "svg2pdf");

async function buildPDF2SVG() {
    try {
        console.log("Building pdf2svg...");
        await exec("cargo build --release --features cli");
        console.log("pdf2svg was build successfully.")
    }   catch (e) {
        throw new Error("Build of pdf2svg failed.")
    }
}

async function buildPDFs() {
    const svgFiles = await glob('**/*.svg', {cwd: svgFilesPath.name});

    for (let filename of svgFiles) {
        let input = path.join(svgFilesPath.name, filename);
        let outputFolder = path.join(pdfFilesPath.name, path.dirname(filename));
        let output = path.join(outputFolder, path.parse(path.basename(filename)).name + ".pdf");
        let command = pdf2svgBinaryPath + ' ' + input + ' ' + output;

        console.log(outputFolder)

        if (!fs.existsSync(outputFolder)){
            fs.mkdirSync(outputFolder, { recursive: true });
        }

        await exec(command);
    }
}


(async function () {

    try {
        await buildPDF2SVG();
        await buildPDFs();
    }   catch (e) {
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