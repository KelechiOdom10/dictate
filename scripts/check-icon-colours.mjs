/**
 * Fails when the app icon's accent has drifted from the colour it claims to be.
 *
 *   node scripts/check-icon-colours.mjs
 *
 * The icon paints each theme's **identity** colour, and that colour has two
 * sources by design:
 *
 * - Ember's is its live/recording orange, the `live` role in
 *   `src/ui/palettes.generated.json`, because that orange is the brand and the
 *   focus ring is only its contrast-safe, desaturated variant.
 * - Paper, Capy, Rose Pine and Neutral have no separate identity colour, so
 *   their focus ring (`--focus-outline-color` in the generated stylesheet) is it.
 *
 * The Rust table cannot import either file, so `app_icon::IDENTITY` is a hand
 * copy. That copy is exactly what goes stale when a theme is re-dialled, which
 * is how this branch once shipped three themes sharing one accent.
 * `pnpm release:mac` runs this before it builds.
 */
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const THEMES = join(root, "src/themes");
const PALETTES = join(root, "src/ui/palettes.generated.json");
const TABLE = join(root, "src-tauri/src/app_icon.rs");

/**
 * Theme slug on disk -> where its identity colour lives.
 * `live` reads the overlay palette; `ring` reads that theme's stylesheet.
 */
const THEMES_BY_SOURCE = {
  ember: { variant: "Ember", from: "live" },
  paper: { variant: "Paper", from: "ring" },
  capy: { variant: "Capy", from: "ring" },
  "rose-pine": { variant: "RosePine", from: "ring" },
  zinc: { variant: "Neutral", from: "ring" },
};

const hex = (value) => {
  const at = value.trim().replace("#", "");
  if (!/^[0-9a-f]{6}$/i.test(at)) throw new Error(`not a hex colour: ${value}`);
  return [0, 2, 4].map((i) => parseInt(at.slice(i, i + 2), 16));
};

const rgb = (value) => {
  const channels = value.match(/rgb\(\s*(\d+)\s+(\d+)\s+(\d+)\s*\)/);
  if (!channels) throw new Error(`not an rgb() colour: ${value}`);
  return channels.slice(1, 4).map(Number);
};

/** slug -> { light, dark } from the ember overlay palette's `live` role. */
function liveColours() {
  const found = {};
  for (const entry of JSON.parse(readFileSync(PALETTES, "utf8"))) {
    if (entry.theme !== "ember") continue;
    const mode = entry.mode === "dark" ? "dark" : "light";
    found[mode] = rgb(entry.colors.live);
  }
  return found;
}

/** slug -> { light, dark } from that theme's generated stylesheet ring. */
function rings() {
  const found = {};
  for (const slug of readdirSync(THEMES)) {
    let css;
    try {
      css = readFileSync(join(THEMES, slug, `${slug}.css`), "utf8");
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

const live = liveColours();
const ring = rings();
const rust = table();
const problems = [];

for (const [slug, { variant, from }] of Object.entries(THEMES_BY_SOURCE)) {
  const source = from === "live" ? live : ring[slug];
  if (!source) {
    problems.push(
      `${slug}: no ${from === "live" ? "ember live" : "stylesheet"} colour found`,
    );
    continue;
  }
  const entry = rust[variant];
  if (!entry) {
    problems.push(`${variant}: missing from app_icon::IDENTITY`);
    continue;
  }
  for (const mode of ["light", "dark"]) {
    if (source[mode].join() !== entry[mode].join()) {
      problems.push(
        `${slug} ${mode} (${from}): source has rgb(${source[mode].join(" ")}), app_icon.rs has rgb(${entry[mode].join(" ")})`,
      );
    }
  }
}

if (problems.length > 0) {
  console.error("icon colours do not match their sources:");
  for (const problem of problems) console.error(`  ${problem}`);
  console.error(
    "\nUpdate IDENTITY in src-tauri/src/app_icon.rs, then run pnpm icons:renders.",
  );
  process.exit(1);
}

console.log(
  `icons: ${Object.keys(THEMES_BY_SOURCE).length} identity colours match their sources (ember live, four rings)`,
);
