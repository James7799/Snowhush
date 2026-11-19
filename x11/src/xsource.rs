//! The windows on X11, from what the window manager keeps on the root and on each app's window (EWMH, as GNOME's
//! mutter does): the stacking order, each window's desktop, state and type, and the invisible borders GTK and
//! mutter draw shadows in. Unlike Wayland, X11 tells any app where windows and the pointer are, so this is the
//! whole source: one thread with its own connection, woken by the server's events (a frame moving, the list
//! changing) and asking the pointer at frame rate while the snow wants it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::log;
use crate::source::{Demand, Display, Panel, Report, Shared, Window};
use crate::xlib::{self, Monitor, X};

/// While the snow follows closely: about a frame.
const LIVE: Duration = Duration::from_millis(16);
/// While no snow shows, the windows are asked now and then anyway.
const CALM: Duration = Duration::from_millis(250);
/// Asked again even without an event, in case one was missed.
const REFRESH: Duration = Duration::from_secs(1);
const MONITORS_EVERY: Duration = Duration::from_secs(2);
/// How long after a window last moved under a held button it still counts as dragged.
const SETTLE: Duration = Duration::from_millis(300);
/// GNOME's windows round their top corners by about this, in logical pixels (libadwaita 12, GTK 3 and mutter's
/// title bars 8).
const RADIUS: f32 = 10.0;
const ALL_DESKTOPS: u64 = 0xffff_ffff;
/// How often GNOME Shell is asked whether its overview (Activities) is open: the overlay is above it, so the
/// snow must fade out of its way, as it does on niri's overview.
const OVERVIEW_EVERY: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// An app's window or dialog: snow settles on it.
    Solid,
    /// A dock or panel.
    Panel,
    /// The desktop itself (GNOME's desktop icons): not a window to the snow.
    Desktop,
    /// Menus, splash screens, notifications: they only hide snow.
    Other,
}

struct Known {
    class: String,
    kind: Kind,
}

pub fn start(shared: Arc<Shared>) -> Result<(), String> {
    let mut x = X::open()?;
    x.quiet_errors();
    x.watch_root();
    let scale = x.scale();
    let _ = x.atom("_NET_CLIENT_LIST_STACKING");
    std::thread::Builder::new().name("x11 windows".into()).spawn(move || run(x, shared, scale)).map_err(|error| error.to_string())?;
    Ok(())
}

fn run(mut x: X, shared: Arc<Shared>, scale: f64) {
    log!("x11: scale {scale}");
    let mut known: HashMap<xlib::Window, Known> = HashMap::new();
    let mut last: Option<Report> = None;
    let mut moved_at: Option<Instant> = None;
    let mut asked_at: Option<Instant> = None;
    let mut monitors = x.monitors();
    let mut monitors_at = Instant::now();
    let mut dirty = true;
    let bus = zbus::blocking::Connection::session().ok();
    let mut overview = false;
    let mut overview_at: Option<Instant> = None;
    loop {
        let demand = shared.windows_demand();
        let pointer_wanted = shared.pointer_wanted();
        let wait = match demand {
            Demand::Live => LIVE,
            Demand::Calm => CALM,
            Demand::Off => REFRESH,
        };
        if x.wait(wait) && x.take_events() > 0 {
            dirty = true;
        }
        let now = Instant::now();
        if demand != Demand::Off && overview_at.is_none_or(|at| now - at >= OVERVIEW_EVERY) {
            overview_at = Some(now);
            let open = bus.as_ref().is_some_and(overview_open);
            dirty |= open != overview;
            overview = open;
        }
        let pointer = x.pointer();
        if pointer_wanted && let Some((px, py, _)) = pointer {
            shared.set_cursor((px as f64 / scale) as f32, (py as f64 / scale) as f32);
        }
        let pressed = pointer.is_some_and(|p| p.2);
        let dragging = moved_at.is_some_and(|at| now - at < SETTLE);
        let due = asked_at.is_none_or(|at| now - at >= REFRESH) || (dragging && demand == Demand::Live);
        if !(dirty || due) || (demand == Demand::Off && !dirty) {
            continue;
        }
        dirty = false;
        asked_at = Some(now);
        if now - monitors_at >= MONITORS_EVERY {
            monitors = x.monitors();
            monitors_at = now;
        }
        let mut report = ask(&mut x, &mut known, &monitors, scale);
        report.hidden = overview;
        if pressed && let Some(previous) = &last
            && moved(previous, &report)
        {
            moved_at = Some(now);
        }
        report.moving = moved_at.is_some_and(|at| now - at < SETTLE);
        if last.as_ref() != Some(&report) {
            shared.publish(report.clone());
            last = Some(report);
        }
    }
}

/// Whether any window that was there before moved or changed size.
fn moved(before: &Report, after: &Report) -> bool {
    after.windows.iter().any(|w| {
        before.windows.iter().any(|b| b.id == w.id && (b.x != w.x || b.y != w.y || b.w != w.w || b.h != w.h))
    })
}

fn ask(x: &mut X, known: &mut HashMap<xlib::Window, Known>, monitors: &[Monitor], scale: f64) -> Report {
    let root = x.root;
    let desktop = x.cardinals(root, "_NET_CURRENT_DESKTOP").and_then(|v| v.first().copied()).unwrap_or(0);
    let stacking = x.cardinals(root, "_NET_CLIENT_LIST_STACKING").unwrap_or_default();
    let gtk_areas = x.cardinals(root, &format!("_GTK_WORKAREAS_D{desktop}")).unwrap_or_default();
    let net_area = x.cardinals(root, "_NET_WORKAREA").unwrap_or_default();
    let l = |v: i64| v as f64 / scale;

    let screens = monitors
        .iter()
        .map(|m| {
            let (mx, my, mw, mh) = (m.x as i64, m.y as i64, m.w as i64, m.h as i64);
            // GNOME keeps one work area per display; others only one for the whole desktop, cut to the display here.
            let inside = gtk_areas.chunks_exact(4).map(|r| (r[0] as i64, r[1] as i64, r[2] as i64, r[3] as i64)).find(|r| {
                r.0 >= mx && r.1 >= my && r.0 + r.2 <= mx + mw && r.1 + r.3 <= my + mh
            });
            let work = inside.or_else(|| {
                let d = desktop as usize * 4;
                net_area.get(d..d + 4).map(|r| {
                    let (x0, y0) = ((r[0] as i64).max(mx), (r[1] as i64).max(my));
                    let (x1, y1) = (((r[0] + r[2]) as i64).min(mx + mw), ((r[1] + r[3]) as i64).min(my + mh));
                    (x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
                })
            });
            let (ax, ay, aw, ah) = work.filter(|w| w.2 > 0 && w.3 > 0).unwrap_or((mx, my, mw, mh));
            Display { name: m.name.clone(), x: l(mx), y: l(my), w: l(mw), h: l(mh), ax: l(ax), ay: l(ay), aw: l(aw), ah: l(ah) }
        })
        .collect();

    let (hidden, fullscreen, above, max_v, max_h) = (
        x.atom("_NET_WM_STATE_HIDDEN"),
        x.atom("_NET_WM_STATE_FULLSCREEN"),
        x.atom("_NET_WM_STATE_ABOVE"),
        x.atom("_NET_WM_STATE_MAXIMIZED_VERT"),
        x.atom("_NET_WM_STATE_MAXIMIZED_HORZ"),
    );
    let listed: HashSet<xlib::Window> = stacking.iter().map(|w| *w as xlib::Window).collect();
    known.retain(|w, _| listed.contains(w));
    let mut windows = Vec::new();
    let mut panels = Vec::new();
    let mut elsewhere = Vec::new();
    for &id in stacking.iter().rev() {
        let w = id as xlib::Window;
        if !known.contains_key(&w) {
            x.watch_window(w);
            let class = x.text(w, "WM_CLASS").and_then(|names| names.last().cloned()).unwrap_or_default();
            let kind = kind_of(x, w);
            known.insert(w, Known { class, kind });
        }
        let info = &known[&w];
        if info.kind == Kind::Desktop {
            continue;
        }
        let name = format!("{w:#x}");
        let on = x.cardinals(w, "_NET_WM_DESKTOP").and_then(|v| v.first().copied()).unwrap_or(ALL_DESKTOPS);
        if on != desktop && on != ALL_DESKTOPS {
            elsewhere.push(name);
            continue;
        }
        let state: Vec<u64> = x.cardinals(w, "_NET_WM_STATE").unwrap_or_default();
        let has = |atom: u64| state.contains(&atom);
        let Some((gx, gy, gw, gh, viewable)) = x.geometry(w) else { continue };
        if !viewable || has(hidden) {
            continue;
        }
        let (mut rx, mut ry, mut rw, mut rh) = (gx as i64, gy as i64, gw as i64, gh as i64);
        // A title bar mutter draws: the window shows that much bigger.
        if let Some(e) = x.cardinals(w, "_NET_FRAME_EXTENTS").filter(|e| e.len() == 4) {
            rx -= e[0] as i64;
            ry -= e[2] as i64;
            rw += (e[0] + e[1]) as i64;
            rh += (e[2] + e[3]) as i64;
        }
        // Shadows a GTK app draws around itself are not the window.
        if let Some(e) = x.cardinals(w, "_GTK_FRAME_EXTENTS").filter(|e| e.len() == 4) {
            rx += e[0] as i64;
            ry += e[2] as i64;
            rw -= (e[0] + e[1]) as i64;
            rh -= (e[2] + e[3]) as i64;
        }
        if rw <= 0 || rh <= 0 {
            continue;
        }
        if info.kind == Kind::Panel {
            panels.push(Panel { id: name, x: l(rx), y: l(ry), w: l(rw), h: l(rh) });
            continue;
        }
        let maximized = has(max_v) && has(max_h);
        let full = has(fullscreen);
        windows.push(Window {
            id: name,
            class: info.class.clone(),
            x: l(rx),
            y: l(ry),
            w: l(rw),
            h: l(rh),
            solid: info.kind == Kind::Solid,
            full,
            above: has(above),
            decorated: true,
            maximized,
            radius: Some(if maximized || full { 0.0 } else { RADIUS }),
        });
    }
    Report { screens, windows, elsewhere, panels, desktop: desktop.to_string(), ..Report::default() }
}

/// Whether GNOME Shell shows its overview; false where there is no GNOME Shell.
fn overview_open(bus: &zbus::blocking::Connection) -> bool {
    bus.call_method(
        Some("org.gnome.Shell"),
        "/org/gnome/Shell",
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.gnome.Shell", "OverviewActive"),
    )
    .ok()
    .and_then(|reply| reply.body().deserialize::<zbus::zvariant::OwnedValue>().ok())
    .and_then(|value| bool::try_from(value).ok())
    .unwrap_or(false)
}

fn kind_of(x: &mut X, w: xlib::Window) -> Kind {
    let types = x.cardinals(w, "_NET_WM_WINDOW_TYPE").unwrap_or_default();
    let Some(&first) = types.first() else { return Kind::Solid };
    let name = x.atom_name(first);
    match name.trim_start_matches("_NET_WM_WINDOW_TYPE_") {
        "NORMAL" | "DIALOG" | "UTILITY" => Kind::Solid,
        "DOCK" => Kind::Panel,
        "DESKTOP" => Kind::Desktop,
        _ => Kind::Other,
    }
}
