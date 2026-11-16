import { copyFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import {
  DMG_NAME,
  DOWNLOADS,
  PAGES_FILE_LIMIT,
  megabytes,
} from "../src/downloads.ts";

const argument = process.argv[2];
if (!argument) {
  console.error("usage: pnpm --dir site add-build <path-to-dmg>");
  process.exit(1);
}
const source = resolve(process.env.INIT_CWD ?? process.cwd(), argument);
const name = basename(source);
if (!DMG_NAME.test(name)) {
  console.error(`${name} is not named dictate-<version>-arm64.dmg`);
  process.exit(1);
}
const bytes = statSync(source).size;
if (bytes > PAGES_FILE_LIMIT) {
  console.error(
    `${name} is ${bytes} bytes, over the 25 MiB Cloudflare Pages file limit`,
  );
  process.exit(1);
}

const folder = join(import.meta.dirname, "..", DOWNLOADS);
for (const old of readdirSync(folder).filter((file) => file.endsWith(".dmg"))) {
  rmSync(join(folder, old));
}
copyFileSync(source, join(folder, name));
console.log(`${DOWNLOADS}/${name}, ${megabytes(bytes)}`);
