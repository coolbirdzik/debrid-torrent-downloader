# DebridDock logo

The cloud and downward arrow represent downloading files from debrid services.
The cyan receiving tray is the dock: one destination for the library and download
queue across providers. Cobalt blue fits the existing interface; white and cyan
keep the symbol visible in light and dark themes. The tile has transparent corners.

`static/app-icon.png` is the source artwork used by the sidebar. Run `pnpm icons`
after replacing it to regenerate the browser favicon and desktop PNG, ICO and ICNS
assets under `src-tauri/icons`. Commit the source and all generated desktop assets
together. Rebuild the desktop application to embed the new executable/bundle icons.

## Generation

Created with the built-in `image_gen` tool, with a transparent background. Prompt:

> Use case: logo-brand. Asset type: final desktop application icon for DebridDock, a download manager that brings cloud torrent libraries from multiple debrid providers into one local download queue. Create ONE finished original symbol, not a presentation sheet. A beautifully simple rounded-square app tile in rich cobalt blue with a restrained brighter blue highlight at upper left. Within it, an exceptionally clear white cloud silhouette at top, whose lower middle extends into a bold downward arrow entering a separate cyan U-shaped receiving dock/tray below. Cloud -> down arrow -> dock: a single cohesive balanced emblem with generous negative space and strong readable geometry at 16px and 32px. All foreground elements optically centered. Tile occupies about 92 percent of the square canvas, rounded corners about 22 percent of tile width, with genuine transparent pixels only outside the tile. Flat vector-like precise smooth edges, minimal subtle tonal depth in tile only, no 3D, no bevels, no glow, no busy lines, no small dots, no lettering, no watermark, no provider brand marks. This is a refined functional software brand for a light and dark glass desktop UI. Square 1024x1024 composition. No mockup, no external cast shadow.
