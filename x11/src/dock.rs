//! Where GNOME's dock shows (Ubuntu Dock, Dash to Dock), for snow to lie on as on a panel. gnome-shell draws it
//! itself, so no X window has it; its input region is the dock's whole container, far wider than what shows. Its
//! accessible objects (AT-SPI) do tell: under the shell's stage, each display's dock container has the dash as its
//! first child, the bar of icons people see. A dock intellihide has put away is still listed, with no height or
//! below the screen.
//! Found once (and again every so often, or when asking fails), then each dash is asked where it is; every call
//! gives up after a moment, so a busy shell can't stall the window source.

use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::log;
use crate::xlib::Monitor;

const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const COMPONENT: &str = "org.a11y.atspi.Component";
/// How long one call may take before the dock counts as unknown.
const PATIENCE: Duration = Duration::from_millis(300);
/// The docks are looked for again this often, and after a failure not before this.
const REDISCOVER: Duration = Duration::from_secs(20);

/// A rectangle in the X server's pixels: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

#[derive(Default)]
pub struct Docks {
    bus: Option<Connection>,
    /// gnome-shell's bus name on the accessibility bus, and each dock's dash: its object path.
    dashes: Vec<(String, String)>,
    found_at: Option<Instant>,
    failed: bool,
}

impl Docks {
    /// The docks showing along the bottom of a display: their dash, cut to the display.
    pub fn showing(&mut self, monitors: &[Monitor]) -> Vec<Rect> {
        let now = Instant::now();
        if self.found_at.is_none_or(|at| now - at >= REDISCOVER) {
            self.found_at = Some(now);
            match self.discover(monitors) {
                Ok(()) => self.failed = false,
                Err(error) => {
                    if !self.failed {
                        log!("dock: not found ({error}); no snow on the dock");
                    }
                    self.failed = true;
                    self.dashes.clear();
                }
            }
        }
        let Some(bus) = &self.bus else { return Vec::new() };
        let mut out = Vec::new();
        let mut broken = false;
        for (name, path) in &self.dashes {
            let Ok(e) = extents(bus, name, path) else {
                broken = true;
                continue;
            };
            let Some(m) = monitors.iter().find(|m| e.0 >= m.x - 2 && e.0 + e.2 <= m.x + m.w + 2 && e.1 < m.y + m.h && e.1 >= m.y) else {
                continue;
            };
            // Sliding away, or put away: only what is still on the display, and only while most of it is.
            let bottom = (e.1 + e.3).min(m.y + m.h);
            let shown = bottom - e.1;
            if e.2 > 0 && shown > 20 && shown * 3 >= e.3 * 2 {
                out.push((e.0, e.1, e.2, shown));
            }
        }
        if broken {
            self.found_at = None;
        }
        out
    }

    fn discover(&mut self, monitors: &[Monitor]) -> Result<(), String> {
        if self.bus.is_none() {
            self.bus = Some(connect()?);
        }
        let bus = self.bus.as_ref().ok_or("no accessibility bus")?;
        let apps = children(bus, "org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root")?;
        let (name, root) = apps
            .into_iter()
            .find(|(name, path)| self::name(bus, name, path).is_ok_and(|n| n == "gnome-shell"))
            .ok_or("gnome-shell isn't on the accessibility bus")?;
        let mut dashes = Vec::new();
        // gnome-shell, its window, the stage, and the stage's children: the docks among them.
        for (_, window) in children(bus, &name, &root)? {
            for (_, stage) in children(bus, &name, &window)? {
                for (_, child) in children(bus, &name, &stage)? {
                    let Ok(e) = extents(bus, &name, &child) else { continue };
                    let is_dock = monitors.iter().any(|m| {
                        let bottom = m.y + m.h;
                        (e.1 + e.3 - bottom).abs() <= 2
                            && e.0 >= m.x - 2
                            && e.0 + e.2 <= m.x + m.w + 2
                            && e.2 >= m.w / 4
                            && e.2 > 3 * e.3.max(1)
                    });
                    if !is_dock {
                        continue;
                    }
                    if let Some((_, dash)) = children(bus, &name, &child)?.into_iter().next() {
                        dashes.push((name.clone(), dash));
                    }
                }
            }
        }
        if dashes.len() != self.dashes.len() {
            log!("dock: {} found", dashes.len());
        }
        self.dashes = dashes;
        Ok(())
    }
}

fn connect() -> Result<Connection, String> {
    let session = Connection::session().map_err(|error| error.to_string())?;
    let reply = session
        .call_method(Some("org.a11y.Bus"), "/org/a11y/bus", Some("org.a11y.Bus"), "GetAddress", &())
        .map_err(|error| error.to_string())?;
    let address: String = reply.body().deserialize().map_err(|error| error.to_string())?;
    zbus::blocking::connection::Builder::address(address.as_str())
        .map_err(|error| error.to_string())?
        .method_timeout(PATIENCE)
        .build()
        .map_err(|error| error.to_string())
}

fn children(bus: &Connection, name: &str, path: &str) -> Result<Vec<(String, String)>, String> {
    let reply = bus.call_method(Some(name), path, Some(ACCESSIBLE), "GetChildren", &()).map_err(|error| error.to_string())?;
    let list: Vec<(String, OwnedObjectPath)> = reply.body().deserialize().map_err(|error| error.to_string())?;
    Ok(list.into_iter().map(|(name, path)| (name, path.as_str().to_string())).collect())
}

fn name(bus: &Connection, name: &str, path: &str) -> Result<String, String> {
    let reply = bus
        .call_method(Some(name), path, Some("org.freedesktop.DBus.Properties"), "Get", &(ACCESSIBLE, "Name"))
        .map_err(|error| error.to_string())?;
    let value: OwnedValue = reply.body().deserialize().map_err(|error| error.to_string())?;
    String::try_from(value).map_err(|error| error.to_string())
}

/// On the screen, in the X server's pixels.
fn extents(bus: &Connection, name: &str, path: &str) -> Result<Rect, String> {
    let reply = bus.call_method(Some(name), path, Some(COMPONENT), "GetExtents", &(0u32,)).map_err(|error| error.to_string())?;
    reply.body().deserialize::<Rect>().map_err(|error| error.to_string())
}
