/**
 * Fails when the app icon's accent has drifted from the theme it claims to
 * follow.
 *
 *   node scripts/check-icon-colours.mjs
 *
 * The Rust icon table cannot import a stylesheet, so `app_icon::RINGS` is a
 * hand copy of each theme's focus ring. That copy is exactly the kind of thing
 * that goes stale when a theme is re-dialled, which is how this branch once
 * shipped three themes sharing one accent. `pnpm release:mac` runs this first.
 */
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const THEMES = join(root, "src/themes");
const TABLE = join(root, "src-tauri/src/app_icon.rs");

/** Theme slug on disk -> the variant name in `app_icon::RINGS`. */
const VARIANTS = {
  ember: "Ember",
  paper: "Paper",
  capy: "Capy",
  "rose-pine": "RosePine",
  zinc: "Neutral",
};

const hex = (value) => {
  const at = value.trim().replace("#", "");
  if (!/^[0-9a-f]{6}$/i.test(at)) throw new Error(`not a hex colour: ${value}`);
  return [0, 2, 4].map((i) => parseInt(at.slice(i, i + 2), 16));
};

/** slug -> { light, dark } from that theme's generated stylesheet. */
function rings() {
  const found = {};
  for (const slug of readdirSync(THEMES)) {
    const dir = join(THEMES, slug);
    let css;
    try {
      css = readFileSync(join(dir, `${slug}.css`), "utf8");
    } catch {
      continue;
    }
    const match = css.match(
      /--focus-outline-color:\s*light-dark\(\s*(#[0-9a-f]{6})\s*,\s*(#[0-9a-f]{6})\s*\)/i,
    );
    if (!match) throw new Error(`no --focus-outline-color in ${slug}.css`);
    found[slug] = { light: hex(match[1]), dark: hex(match[2]) };
  }
  return found;
}

/** variant -> { light, dark } as the Rust table states them. */
function table() {
  const source = readFileSync(TABLE, "utf8");
  const found = {};
  const pattern =
    /\(Theme::(\w+),\s*\[(\d+),\s*(\d+),\s*(\d+)\],\s*\[(\d+),\s*(\d+),\s*(\d+)\]\)/g;
  for (const [, variant, ...channels] of source.matchAll(pattern)) {
    const numbers = channels.map(Number);
    found[variant] = { light: numbers.slice(0, 3), dark: numbers.slice(3, 6) };
  }
  return found;
}

const stylesheet = rings();
const rust = table();
const problems = [];

for (const [slug, variant] of Object.entries(VARIANTS)) {
  const theme = stylesheet[slug];
  if (!theme) {
    problems.push(
      `${slug}: no stylesheet found, so the ring cannot be checked`,
    );
    continue;
  }
  const entry = rust[variant];
  if (!entry) {
    problems.push(`${variant}: missing from app_icon::RINGS`);
    continue;
  }
  for (const mode of ["light", "dark"]) {
    if (theme[mode].join() !== entry[mode].join()) {
      problems.push(
        `${slug} ${mode}: theme has rgb(${theme[mode].join(" ")}), app_icon.rs has rgb(${entry[mode].join(" ")})`,
      );
    }
  }
}

if (problems.length > 0) {
  console.error("icon colours do not match the themes:");
  for (const problem of problems) console.error(`  ${problem}`);
  console.error(
    "\nUpdate RINGS in src-tauri/src/app_icon.rs, then run pnpm icons:renders.",
  );
  process.exit(1);
}

console.log(
  `icons: ${Object.keys(VARIANTS).length} theme rings match app_icon.rs`,
);
