# dictate.kellycooks.dev

The one-page download site for dictate. It is a static Astro build: one HTML
file with its CSS inlined, one self-hosted font (Geist, 28 KB), no analytics
and no third-party requests. The site deploys as a Cloudflare Worker with
static assets (`site/wrangler.jsonc`), serving the DMG under `/downloads/` and
update artifacts under `/updates/`.

```sh
pnpm --dir site install
pnpm --dir site dev        # http://localhost:4321
pnpm --dir site build      # astro check, then dist/
pnpm --dir site deploy     # astro build && wrangler deploy
```

## Shipping a new build

The page links to the one DMG in `site/public/downloads/` and prints its size,
both read at build time, so a new release swaps the DMG from the repository
root:

```sh
pnpm --dir site add-build release/dictate-0.2.0-arm64.dmg
```

It removes the old DMG, copies the new one in and prints its size. A release
also copies its updater manifest (`latest.json`) and update tarball into
`site/public/updates/`, which Astro copies verbatim to `dist/updates/` so the
app's updater can fetch `https://dictate.kellycooks.dev/updates/latest.json`.
Commit the changes and push to `main`. Cloudflare then rebuilds and deploys the
Worker, and the page shows the new version, file name and size.

The DMG in this folder now is `dictate-0.1.0-arm64.dmg` from PR #45
(`85b37ec`), ad-hoc signed. Replace it with a fresh artifact every release.

The build fails rather than shipping a wrong page if the folder holds no DMG,
more than one, a file not named `dictate-<version>-arm64.dmg`, or a file over
25 MiB. Cloudflare Workers static assets reject any single file over 25 MiB;
today's DMG is 7.6 MB, so there is about 17 MB of headroom. Past that limit the
DMG has to move to R2 or GitHub Releases.

Each DMG is committed to git, so the repository grows by its size every
release. That is fine at 7.6 MB for a handful of releases. If it starts to hurt,
move the DMG to R2.

## The settings screenshots

`public/app/settings-{light,dark}.webp` (1440 px, 2× the 720 × 580 settings
window) and their 720 px versions are Storybook captures of the
`Pages / General Fresh Profile` story in Ember, taken from PR #48 at
`5f19989` and re-shot from `main` at `8f0d77c`, where they came out byte-identical (the General page did not change). The page shows the light or dark capture to match the visitor's
mode. Retake both when the settings window changes.

## Deploying on Cloudflare (Workers Builds)

Cloudflare retired the separate Pages flow; the site deploys as a Worker with
static assets configured by `site/wrangler.jsonc`. You do these steps once in
the Cloudflare dashboard, signed in to the account that holds `kellycooks.dev`.
(The exact labels and field order in the dashboard move around, so treat the
two setups below according to whether a root directory field is shown.)

1. **Workers & Pages → Create → Workers → Connect to Git.** Pick GitHub, allow
   access to `KelechiOdom10/dictate`, and select it.
2. **Configure the build:**
   - Project / Worker name: `dictate` (matches `"name": "dictate"` in
     `site/wrangler.jsonc`)
   - Production branch: `main`
   - **If the form has a Root directory field:** set Root directory to `site`,
     Build command to `pnpm build`, and Deploy command to `npx wrangler deploy`.
   - **If there is no Root directory field (runs from repo root):** set Build
     command to `pnpm --dir site install --frozen-lockfile && pnpm --dir site build`
     and Deploy command to `npx wrangler deploy --config site/wrangler.jsonc`.
   - No environment variables are needed. `site/.node-version` pins Node 24,
     and `wrangler` is pinned in `site/package.json`.
3. **Save and Deploy.** Once the first build finishes, open the `*.workers.dev`
   URL and check that the download button serves the DMG.
4. **Settings → Domains & Routes → Add → Custom domain →
   `dictate.kellycooks.dev`.** Because `kellycooks.dev` is already a zone on
   this Cloudflare account, Cloudflare attaches the route and issues the TLS
   certificate automatically without adding DNS records by hand.
5. **Optional: Settings → Build → Build watch paths.** Include `site/*` so
   pushes that touch only the app do not rebuild the site.

`public/_headers` makes Cloudflare serve the DMG as a download
(`Content-Disposition: attachment`) with a five-minute cache, so a new build
shows up quickly. Hashed assets get a one-year immutable cache.

## Credits

Geist is under the SIL Open Font License (`src/assets/fonts/OFL.txt`). The
small icons are paths from Lucide (ISC licence).
