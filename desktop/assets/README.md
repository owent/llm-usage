# Application identity and static assets

<a id="应用标识与静态资源"></a>

<a id="设计合同"></a>

<a id="设计说明"></a>

<a id="design-contract"></a>

## Design specifications

The mark is **Usage U**: the open U represents usage; central ticks represent observed quantities.
An amber dot above provides visual recognition, without connection/live-collection/zero-usage semantics.
A rounded deep-ink background carries the teal mark; monochrome versions keep the U/ticks and omit the dot.
This repository's original SVG geometry uses no external images, fonts, brand marks or runtime requests.

| Color | Value | Purpose |
| --- | --- | --- |
| Ink | `#102A35` | Icon background/light-theme strokes |
| Teal | `#53DDC5` | U body |
| Pale mint | `#D8FFF3` | Central ticks |
| Amber | `#F2B86B` | Brand recognition only, without status meaning |
| Gray green | `#77958F` | Illustration supporting strokes |

Below 24 px, use the dedicated favicon/monochrome tray mark. Brand icons contain no text or essential
information conveyed only by gradients/color. Illustrations contain no copy; UI text explains statuses.
The preview displays light/dark backgrounds, original small sizes and three empty states.
See [validation](../../docs/validation/desktop-usage/static-assets.md) for browser, Windows package and LFS checks.

<a id="文件与使用"></a>

## Files and consumers

| File | Consumer |
| --- | --- |
| `app-icon.svg` | Authoritative 1024 × 1024 source; regenerate derivatives after changes |
| `tray-dark.svg`, `tray-light.svg` | 16 × 16 tray sources for light/dark backgrounds respectively |
| `../src-tauri/icons/` | PNG/ICO/ICNS referenced by Tauri; retain generated Store/mobile companions without implying platform support |
| `../src-tauri/icons/tray/` | 16/20/24/32/48 px monochrome PNG assets; macOS black template artwork is an asset, not an implemented tray promise |
| `../public/brand/` | Generated SVG and 256/512/1024 px PNG; current window branding uses SVG |
| `../public/favicon.svg`, `../public/favicon.ico` | Browser tabs; SVG is optimized separately for small sizes, ICO is fallback |
| `../public/ui/` | 24 px navigation/action strokes; SVG responds to system theme, CSS masks support forced themes |
| `../public/illustrations/` | No sources/no filter matches/limited sources assets; assets alone do not verify collection functionality |
| `../asset-preview.html` | Local review page at /asset-preview.html after `npm --prefix desktop run dev`; outside release entry |

Callers provide accessible names for icon controls. Images beside equivalent text use `alt=""`.
Empty-state copy examples: No data sources yet; No records match the current filters; Some sources are
temporarily unreadable. Distinguish missing records, read failures and actual zero usage.
Windows opt-in close-to-tray is implemented in `../src-tauri/src/tray.rs`; it currently uses the application
window icon. Dedicated monochrome resources and other platforms do not establish additional tray integration.

<a id="生成和检查"></a>

## Generation and checks

From the repository root with Node.js 22+, use desktop's locked `@tauri-apps/cli` (currently 2.12.0).
No additional drawing dependencies:

```powershell
git lfs install --local
git lfs pull
npm --prefix desktop ci
npm run assets:generate
npm run assets:check
```

Generation rebuilds only the listed derivatives, retaining UI/illustration sources. Repetition should
produce identical bytes. CLI 2.11.5 originally emitted nondeterministic ICNS layer ordering; the script
sorts types while keeping legacy RGB/transparency pairs and unchanged layer content. Render into temporary
directories before copying targets. Failures preserve original icons and return nonzero.
Checks verify sources, PNG format/dimensions, ICO/ICNS directories, derivative consistency and LFS attributes.
Undownloaded pointers fail immediately. Use --help for parameters; assets:check never changes assets.
Rollback restores SVG sources/derivatives together, or regenerates from restored sources.

## Git LFS

Root .gitattributes manages images including SVG, fonts, media, archives, executables, libraries, databases
and Python bytecode. All desktop/public/, Tauri icons and prototype prebuilt JS/static JSON also use LFS.
TS/Svelte/CSS sources, configuration, documentation and generators use ordinary Git for review.
Build/dependency directories remain ignored. LFS rules do not justify tracking packages/caches/local data.
Already tracked prototype databases/bytecode changed storage only, without content changes/new data.

Migration uses .gitattributes and targeted git add --renormalize to convert the current index, without
rewriting history. Historical ordinary blobs remain. New git add operations save pointers while working
files remain directly usable. CI checkout requires lfs: true; check assets before use to avoid packaging
pointers as images.

Sources verified on 2026-09-24:
[Tauri icons](https://v2.tauri.app/develop/icons/),
[Git LFS 3.7.1 migration](https://github.com/git-lfs/git-lfs/blob/v3.7.1/docs/man/git-lfs-migrate.adoc),
[pinned checkout LFS input](https://github.com/actions/checkout/blob/d23441a48e516b6c34aea4fa41551a30e30af803/README.md).
