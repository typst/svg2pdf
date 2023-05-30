/* Copyright 2017 Mozilla Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// Adapted from https://github.com/mozilla/pdf.js/blob/master/examples/node/pdf2png/pdf2png.js
import assert from "assert";
import {Canvas as CanvasType} from "canvas";

const Canvas = require("canvas");
const pdfjs = require("pdfjs-dist/legacy/build/pdf.js");

interface CanvasAndContext {
    canvas: CanvasType | null;
    context: CanvasRenderingContext2D | null;
}


class NodeCanvasFactory {
    create(width: number, height: number): CanvasAndContext {
        assert(width > 0 && height > 0, "Invalid canvas size");
        const canvas = Canvas.createCanvas(width, height);
        const context = canvas.getContext("2d");
        return {
            canvas,
            context,
        };
    }

    reset(canvasAndContext: CanvasAndContext, width: number, height: number): void {
        assert(canvasAndContext.canvas, "Canvas is not specified");
        assert(width > 0 && height > 0, "Invalid canvas size");
        canvasAndContext.canvas.width = width;
        canvasAndContext.canvas.height = height;
    }

    destroy(canvasAndContext: CanvasAndContext): void {
        assert(canvasAndContext.canvas, "Canvas is not specified");

        // Zeroing the width and height cause Firefox to release graphics
        // resources immediately, which can greatly reduce memory consumption.
        canvasAndContext.canvas.width = 0;
        canvasAndContext.canvas.height = 0;
        canvasAndContext.canvas = null;
        canvasAndContext.context = null;
    }
}

// Some PDFs need external cmaps.
const CMAP_URL = "../node_modules/pdfjs-dist/cmaps/";
const CMAP_PACKED = true;

// Where the standard fonts are located.
const STANDARD_FONT_DATA_URL =
    "../node_modules/pdfjs-dist/standard_fonts/";

const canvasFactory = new NodeCanvasFactory();

async function convertPDFtoPNG(pdfFile: Uint8Array): Promise<Uint8Array> {

    // Load the PDF file.
    const loadingTask = pdfjs.getDocument({
        data: pdfFile,
        cMapUrl: CMAP_URL,
        cMapPacked: CMAP_PACKED,
        standardFontDataUrl: STANDARD_FONT_DATA_URL,
        canvasFactory,
    });

    const document = await loadingTask.promise;
    const page = await document.getPage(1);
    const viewport = page.getViewport({scale: 2.5});
    const canvasAndContext = canvasFactory.create(
        viewport.width,
        viewport.height
    );
    const renderContext = {
        canvasContext: canvasAndContext.context,
        background: 'rgba(0,0,0,0)',
        viewport,
    };

    const renderTask = page.render(renderContext);
    await renderTask.promise;

    const image = (canvasAndContext.canvas as CanvasType).toBuffer();
    page.cleanup();

    return image;
}

export {convertPDFtoPNG}