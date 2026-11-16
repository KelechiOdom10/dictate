import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import {
  DMG_NAME,
  DOWNLOADS,
  PAGES_FILE_LIMIT,
  megabytes,
} from "./downloads.ts";

function readRelease() {
  const folder = join(process.cwd(), DOWNLOADS);
  const dmgs = readdirSync(folder).filter((file) => file.endsWith(".dmg"));
  if (dmgs.length !== 1) {
    throw new Error(
      `${DOWNLOADS} must hold exactly one DMG, found ${dmgs.length}. Run pnpm add-build <path-to-dmg>.`,
    );
  }
  const [file] = dmgs;
  const version = DMG_NAME.exec(file)?.[1];
  if (!version)
    throw new Error(`${file} is not named dictate-<version>-arm64.dmg`);
  const bytes = statSync(join(folder, file)).size;
  if (bytes > PAGES_FILE_LIMIT) {
    throw new Error(
      `${file} is ${bytes} bytes, over the 25 MiB Cloudflare Pages file limit`,
    );
  }
  return { file, version, href: `/downloads/${file}`, size: megabytes(bytes) };
}

export const release = readRelease();
