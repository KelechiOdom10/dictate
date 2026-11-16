export const DOWNLOADS = "public/downloads";
export const PAGES_FILE_LIMIT = 25 * 1024 * 1024;
export const DMG_NAME = /^dictate-(\d+\.\d+\.\d+)-arm64\.dmg$/;
export const megabytes = (bytes: number) =>
  `${(bytes / 1_000_000).toFixed(1)} MB`;
