import { launch } from "./browser.mjs";
import { readdirSync, readFileSync } from "node:fs";

const dir = new URL("../../public/app/", import.meta.url).pathname;
const files = readdirSync(dir).filter((f) => /^take-(clean|ink)[^.]*\.webp$/.test(f));
const browser = await launch();
const page = await browser.newPage();
let failed = false;
for (const file of files) {
  const b64 = readFileSync(dir + file).toString("base64");
  const r = await page.evaluate(async (b64) => {
    const img = new Image();
    img.src = "data:image/webp;base64," + b64;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0);
    const y = Math.round(img.height * 0.065);
    const row = g.getImageData(0, y, img.width, 1).data;
    const dark = (x) => row[x * 4] < 60 && row[x * 4 + 1] < 60 && row[x * 4 + 2] < 60;
    const xs = [];
    for (let x = 0; x < img.width; x++) if (dark(x)) xs.push(x);
    return { width: img.width, left: xs[0], right: xs[xs.length - 1] };
  }, b64);
  const offset = (r.left + r.right + 1) / 2 - r.width / 2;
  const ok = Math.abs(offset) <= 1;
  if (!ok) failed = true;
  console.log((ok ? "ok   " : "FAIL ") + file + " capsule " + r.left + "-" + r.right + " of " + r.width + ", offset " + offset.toFixed(2) + "px");
}
await browser.close();
process.exit(failed ? 1 : 0);
