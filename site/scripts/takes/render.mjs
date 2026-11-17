const { chromium } = await import(process.env.PLAYWRIGHT_CORE || "playwright-core");
import { writeFileSync, readFileSync } from "node:fs";
const here = new URL(".", import.meta.url).pathname;
const out = process.argv[2] ?? here + "../../public/app";
const b = await chromium.launch(); const exp = await b.newPage();
const jobs = [["d", 965, 508, [[2, ""], [1, "-965"]]], ["m", 315, 280, [[3, "-m945"], [2, "-m630"]]]];
for (const [mk, w, h, scales] of jobs) for (const [name, ink, time] of [["clean", false, "00:38"], ["ink", true, "00:41"]]) for (const [s, suffix] of scales) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: s });
  const p = await ctx.newPage();
  await p.setContent(readFileSync(here + "mock-" + mk + ".html", "utf8").replace('class="MODE"', ink ? 'class="ink"' : ""), { waitUntil: "load" });
  await p.evaluate(async (t) => { await document.fonts.ready; document.querySelector(".t").textContent = t; }, time);
  const png = await p.screenshot();
  const data = await exp.evaluate(async (b64) => { const i = new Image(); i.src = "data:image/png;base64," + b64; await i.decode(); const c = document.createElement("canvas"); c.width = i.width; c.height = i.height; c.getContext("2d").drawImage(i, 0, 0); return c.toDataURL("image/webp", 0.95); }, png.toString("base64"));
  const buf = Buffer.from(data.split(",")[1], "base64");
  writeFileSync(out + "/take-" + name + suffix + ".webp", buf);
  console.log(name + suffix, w * s + "x" + h * s, buf.length);
  await ctx.close();
}
await b.close();
