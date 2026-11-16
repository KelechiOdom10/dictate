# dictate.kellycooks.dev

The one-page download site for dictate. It is a static Astro build: one HTML
file with its CSS inlined, one self-hosted font (Geist, 28 KB), no analytics
and no third-party requests. The DMG is served from the same Cloudflare Pages
project under `/downloads/`.

```sh
pnpm --dir site install
pnpm --dir site dev        # http://localhost:4321
pnpm --dir site build      # astro check, then dist/
```

## Shipping a new build

The page links to the one DMG in `site/public/downloads/` and prints its size,
both read at build time, so a new release is one command from the repository
root:

```sh
pnpm --dir site add-build release/dictate-0.2.0-arm64.dmg
```

It removes the old DMG, copies the new one in and prints its size. Commit
the change and push it to `main`. Cloudflare then rebuilds the site, and the page shows the new
version, file name and size.

The DMG in this folder now is `dictate-0.1.0-arm64.dmg` from PR #45
(`85b37ec`), ad-hoc signed. Replace it with a fresh artifact every release.

The build fails rather than shipping a wrong page if the folder holds no DMG,
more than one, a file not named `dictate-<version>-arm64.dmg`, or a file over
25 MiB. Cloudflare Pages rejects any single file over 25 MiB; today's DMG is
7.6 MB, so there is about 17 MB of headroom. Past that limit the DMG has to
move to R2 or GitHub Releases.

Each DMG is committed to git, so the repository grows by its size every
release. That is fine at 7.6 MB for a handful of releases. If it starts to hurt,
move the DMG to R2.

## Deploying on Cloudflare Pages

You do these steps once, in the Cloudflare dashboard, signed in to the account
that holds `kellycooks.dev`.

1. **Workers & Pages → Create application → Pages → Connect to Git.** Pick
   GitHub, allow access to `KelechiOdom10/dictate` (private repositories are
   supported), and select it.
2. **Set up builds and deployments:**
   - Project name: `dictate` (this becomes `dictate.pages.dev`, or a variant
     if taken)
   - Production branch: `main`
   - Framework preset: **Astro**
   - Build command: `pnpm build`
   - Build output directory: `dist`
   - Root directory (advanced): `site`
   - No environment variables are needed. `site/.node-version` pins Node 24,
     and the build image's pnpm handles the lockfile.
3. **Save and Deploy.** The first build takes about a minute. Open the
   `*.pages.dev` URL and check that the download button gets the DMG.
4. **Custom domains → Set up a custom domain → `dictate.kellycooks.dev` →
   Continue → Activate domain.** Because `kellycooks.dev` is already a zone on
   this Cloudflare account, Cloudflare adds the proxied `CNAME dictate →
<project>.pages.dev` record to the zone and issues the TLS certificate.
   You add no DNS records by hand. Do not create the CNAME yourself first: a
   record that exists before the domain is attached to the project returns 522.
   Activation usually takes a few minutes.
5. **Optional: Settings → Builds → Build watch paths.** Include `site/*` so
   pushes that touch only the app do not rebuild the site. Every other branch
   push otherwise creates a preview deployment.

`public/_headers` makes Cloudflare serve the DMG as a download
(`Content-Disposition: attachment`) with a five-minute cache, so a new build
shows up quickly. Hashed assets get a one-year immutable cache.

## Credits

Geist is under the SIL Open Font License (`src/assets/fonts/OFL.txt`). The
small icons are paths from Lucide (ISC licence).
