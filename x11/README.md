# Snowhush on X11 (GNOME on Xorg)

A port of `linux/` (Wayland only: KDE Plasma 6, Hyprland, Sway, niri) to X11, for personal use on Ubuntu 24.04
with GNOME on Xorg. Snowhush is PolyForm Noncommercial 1.0.0 (see `../LICENSE`): use and change it freely, do not
sell it or anything built on it, and keep the `Required Notice:` line.

## What changed

The snow physics (`core/`) and every Linux module that does not touch Wayland are used as they are, through
`#[path]`: `chores.rs`, `light.rs`, `log.rs`, `prefs.rs`, `tray.rs`, `windows.rs`. The X11 code is new or patched:

| File | What it is |
|---|---|
| `xlib.rs` | X server through x11-dl (loaded at run time): RandR monitors and refresh, Xft.dpi scale, see-through override-redirect overlays with an empty input shape (every click goes through), EWMH properties |
| `xsource.rs` | The window source: `_NET_CLIENT_LIST_STACKING`, `_NET_WM_DESKTOP/STATE/WINDOW_TYPE`, `_NET_FRAME_EXTENTS`, `_GTK_FRAME_EXTENTS`, GNOME's per-display `_GTK_WORKAREAS_D<n>`, the pointer (XQueryPointer), drags (geometry moving under a held button), GNOME's overview (`org.gnome.Shell OverviewActive`), published as linux/'s `Report` |
| `screen.rs` | `linux/src/screen.rs` with the layer surface swapped for one overlay window per display, placed over its work area at once; drawn in X pixels, simulated in logical pixels |
| `snow.rs` | `linux/src/snow.rs` with Wayland's registry, outputs and frame callbacks swapped for RandR and X events |
| `render.rs` | `linux/src/render.rs` with EGL on the X11 platform, a config whose native visual is 32-bit ARGB, and surfaces on X windows |
| `source.rs` | `linux/src/source.rs` with the compositor sources swapped for `xsource` |
| `kwin.rs` | Only the single-instance D-Bus name and the `Slide` type |
| `dock.rs` | GNOME's dock (Ubuntu Dock, Dash to Dock) from gnome-shell's accessible objects (AT-SPI): each display's dash, which becomes a panel the snow lies on, its top the work area's bottom |
| `tray.rs` | `linux/src/tray.rs` with **⛄ Build a Snowman** added |

## Snowmen

**Build one by hand**, as in the video of Tymor (a Mac app): **Ctrl + drag** in the drift along the bottom scoops up
a snowball; dragged along the drift it rolls and grows, taking the snow it rolls over. Let go, it falls and sits in
the drift. Drag a ball (no Ctrl needed: a ball takes the click itself) over another and let go: it lands on top.
Three stacked become a snowman with the balls' own sizes: coal eyes, a smile and buttons, a carrot nose, a red
scarf, twig arms and a black top hat. Grabbing a ball under another drops the one on top.

**⛄ Build a Snowman** in the tray menu builds one by itself on the display under the pointer.

Snowmen and balls stand in the drift (`core/src/simulation/snowballs.rs`, `snowmen.rs`), which lies over the
windows, so a window does not hide them; Clear Snow takes the balls and knocks the snowmen over. They are drawn
procedurally, with no pictures (`src/shaders/snowman.frag`, `snowball.frag`). Over the dock they stand in the
snow lying on it.

## The dock

gnome-shell draws the dock itself, so no X window says where it is, and the shell's input region is the dock's whole
container, much wider than the bar of icons. Its accessible objects do say: `dock.rs` finds each display's dock
under the shell's stage on the accessibility bus and asks where its dash is five times a second. A dash showing
along a display's bottom is reported as a panel, so the snow settles on it as on a KDE panel and falls off when
intellihide puts the dock away. The dock takes every click over itself, so a ball is scooped from the snow on its
top, not from the ground behind it. The overlay takes the pointer only
over the balls, and over the drift while Ctrl is held; everywhere else every click goes through.

Not on X11: Show in Screenshots (the snow always shows in them), and Behind Windows (the overlay is always above
the windows; over full-screen apps it fades out unless Show over Full Screen is on).

## Build and install

```sh
cd x11 && cargo build --release        # ../target or CARGO_TARGET_DIR
install -Dm755 target/release/snowhush-x11 ~/.local/bin/snowhush
```

The menu entry is `~/.local/share/applications/snowhush.desktop`; Open at Login in the tray menu writes
`~/.config/autostart/snowhush.desktop`. Log: `~/.local/state/snowhush/snowhush.log`. `SNOWHUSH_SCALE` overrides the
scale read from Xft.dpi.
