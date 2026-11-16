/**
 * Builds the macOS app icon set from one of the SVG drafts in docs/icon-drafts.
 *
 *   node scripts/build-icons.mjs              # build SELECTED into src-tauri/icons
 *   node scripts/build-icons.mjs --variant c  # build a different draft
 *   node scripts/build-icons.mjs --renders target/icon-renders
 *                                             # render every draft for comparison
 *
 * Swapping the shipped icon is one edit: change SELECTED below.
 *
 * The drafts are drawn on the macOS 1024 grid (an 824 px rounded square inset
 * 100 px with ~185 px corners), so the source needs no extra padding. `tauri
 * icon` rasterises the SVG at every size macOS asks for and packs an .icns that
 * holds 16, 32, 128, 256 and 512 pt at 1x and 2x.
 */
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

/** The shipped icon. Change this line to ship a different draft. */
const SELECTED = "a";

/** Draft id -> source SVG. Keep in sync with docs/distribution.md §5. */
const DRAFTS = {
  a: "docs/icon-drafts/a-ember-wave.svg",
  b: "docs/icon-drafts/b-speech-caret.svg",
  c: "docs/icon-drafts/c-ember-d.svg",
};

/** The files the macOS bundle reads, plus the raster set tauri.conf.json lists. */
const BUNDLE_FILES = [
  "icon.icns",
  "icon.png",
  "icon.ico",
  "32x32.png",
  "128x128.png",
  "128x128@2x.png",
  "Square30x30Logo.png",
  "Square44x44Logo.png",
  "Square71x71Logo.png",
  "Square89x89Logo.png",
  "Square107x107Logo.png",
  "Square142x142Logo.png",
  "Square150x150Logo.png",
  "Square284x284Logo.png",
  "Square310x310Logo.png",
  "StoreLogo.png",
];

const root = resolve(import.meta.dirname, "..");

function generate(svg, outDir) {
  mkdirSync(outDir, { recursive: true });
  run(
    "pnpm",
    ["exec", "tauri", "icon", svg, "--output", outDir],
    `tauri icon failed for ${svg}`,
  );
}

function run(command, args, failure) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "pipe",
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(
      `${failure ?? `${command} failed`}\n${result.stdout}\n${result.stderr}`,
    );
  }
}

/** Renders one draft into a scratch dir and returns it. */
function rasterise(draft) {
  const svg = DRAFTS[draft];
  if (!svg)
    throw new Error(
      `unknown draft "${draft}"; pick one of ${Object.keys(DRAFTS).join(", ")}`,
    );
  const scratch = mkdtempSync(join(tmpdir(), `dictate-icon-${draft}-`));
  generate(svg, scratch);
  return scratch;
}

function buildSelected(draft) {
  const scratch = rasterise(draft);
  const icons = join(root, "src-tauri/icons");
  try {
    for (const file of BUNDLE_FILES)
      copyFileSync(join(scratch, file), join(icons, file));
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
  console.log(
    `icons: built draft ${draft} (${DRAFTS[draft]}) into src-tauri/icons/`,
  );
}

/** Every draft rendered at the sizes a person actually sees it. */
function renderOptions(outDir) {
  const out = resolve(root, outDir);
  mkdirSync(out, { recursive: true });
  for (const draft of Object.keys(DRAFTS)) {
    const scratch = rasterise(draft);
    try {
      // The largest bitmap anywhere in the set is inside the .icns, at 512 pt
      // @2x. Pull it back out for the full-size render.
      const iconset = join(scratch, "icon.iconset");
      run("iconutil", [
        "-c",
        "iconset",
        join(scratch, "icon.icns"),
        "-o",
        iconset,
      ]);
      // 1024: the artboard. 128: one Dock tile on a Retina display. 32: the
      // Settings sidebar and the Finder list at 1x.
      const sizes = [
        ["1024", join(iconset, "icon_512x512@2x.png")],
        ["dock-128", join(scratch, "128x128.png")],
        ["small-32", join(scratch, "32x32.png")],
      ];
      for (const [label, file] of sizes) {
        copyFileSync(file, join(out, `${draft}-${label}.png`));
      }
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  }
  // The menu-bar glyph is drawn in code (src-tauri/src/surface.rs), so this copy
  // is only for comparing the family; it is not a bundle asset.
  writeFileSync(
    join(out, "tray-template.svg"),
    readFileSync(join(root, "docs/icon-drafts/tray-template.svg")),
  );
  console.log(
    `icons: rendered ${Object.keys(DRAFTS).length} drafts into ${outDir}`,
  );
}

const args = process.argv.slice(2);
const flag = (name) => {
  const at = args.indexOf(name);
  return at === -1 ? undefined : (args[at + 1] ?? "");
};

const renders = flag("--renders");
if (renders !== undefined) {
  if (!renders) throw new Error("--renders needs an output directory");
  renderOptions(renders);
} else {
  buildSelected(flag("--variant") ?? SELECTED);
}
