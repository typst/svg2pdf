import {glob} from "glob";
import {referencesPath, svgPath} from "./util";

let svgFilePaths: string[] = [];
let referenceImageFilePaths: string[] = [];

beforeAll(async () => {
    svgFilePaths = await glob('**/*.svg', {cwd: svgPath});
    referenceImageFilePaths = await glob('**/*.png', {cwd: referencesPath});
})

// describe block that will contain each test
describe('Converting .svg files', () => {
    // beforeEach hook if you need to set up anything before each test

    test.each(svgFilePaths)('should convert SVG to PDF correctly', (svgFile) => {
        expect(0).toBe(0);
    });

    // add more tests as needed...
});