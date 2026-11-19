//! The X server: displays from RandR, the desktop's scale, see-through overlay windows that let every click
//! through, and the EWMH properties the window manager keeps on the root and on each app's window.
//! Loaded at run time (x11-dl), so nothing but the X libraries every desktop has is needed.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_int, c_long, c_uchar, c_ulong, c_void};
use std::ptr;

use x11_dl::xfixes::Xlib as XFixes;
use x11_dl::xlib::{self, Xlib};
use x11_dl::xrandr::Xrandr;

pub type Window = xlib::Window;

const SHAPE_INPUT: c_int = 2;

pub struct X {
    pub xlib: Xlib,
    xfixes: XFixes,
    xrandr: Xrandr,
    pub display: *mut xlib::Display,
    pub root: Window,
    atoms: HashMap<String, xlib::Atom>,
}

/// A button press, a drag or a release on one of our overlays, in root coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub window: Window,
    pub kind: PointerKind,
    pub x: i32,
    pub y: i32,
    /// The modifier keys and buttons held (Xlib's state mask).
    pub state: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerKind {
    Press,
    Drag,
    Release,
}

pub const CONTROL: u32 = xlib::ControlMask;

/// A display as RandR's monitors give it, in the X server's pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
    pub refresh: u32,
}

// Each X connection is used by one thread only: the snow loop's for the overlays and EGL, the window source's for
// what the window manager says.
unsafe impl Send for X {}

impl X {
    /// Once per process, before any connection: EGL drivers talk to the server from threads of their own.
    pub fn init_threads() -> Result<(), String> {
        let xlib = Xlib::open().map_err(|error| format!("libX11: {error}"))?;
        unsafe { (xlib.XInitThreads)() };
        Ok(())
    }

    pub fn open() -> Result<X, String> {
        let xlib = Xlib::open().map_err(|error| format!("libX11: {error}"))?;
        let xfixes = XFixes::open().map_err(|error| format!("libXfixes: {error}"))?;
        let xrandr = Xrandr::open().map_err(|error| format!("libXrandr: {error}"))?;
        let display = unsafe { (xlib.XOpenDisplay)(ptr::null()) };
        if display.is_null() {
            return Err("can't open the X display (is DISPLAY set?)".into());
        }
        let root = unsafe { (xlib.XDefaultRootWindow)(display) };
        Ok(X { xlib, xfixes, xrandr, display, root, atoms: HashMap::new() })
    }

    pub fn atom(&mut self, name: &str) -> xlib::Atom {
        if let Some(atom) = self.atoms.get(name) {
            return *atom;
        }
        let c = CString::new(name).expect("an atom name");
        let atom = unsafe { (self.xlib.XInternAtom)(self.display, c.as_ptr(), xlib::False) };
        self.atoms.insert(name.to_string(), atom);
        atom
    }

    pub fn atom_name(&self, atom: xlib::Atom) -> String {
        unsafe {
            let name = (self.xlib.XGetAtomName)(self.display, atom);
            if name.is_null() {
                return String::new();
            }
            let text = CStr::from_ptr(name).to_string_lossy().into_owned();
            (self.xlib.XFree)(name.cast());
            text
        }
    }

    pub fn connection_fd(&self) -> c_int {
        unsafe { (self.xlib.XConnectionNumber)(self.display) }
    }

    pub fn flush(&self) {
        unsafe { (self.xlib.XFlush)(self.display) };
    }

    /// The desktop's scale: GNOME on X11 scales apps itself and says how much through Xft.dpi (192 = 2x).
    /// SNOWHUSH_SCALE overrides it.
    pub fn scale(&self) -> f64 {
        if let Some(scale) = std::env::var("SNOWHUSH_SCALE").ok().and_then(|v| v.parse::<f64>().ok()).filter(|s| *s >= 0.5) {
            return scale;
        }
        let resources = unsafe { (self.xlib.XResourceManagerString)(self.display) };
        if resources.is_null() {
            return 1.0;
        }
        let text = unsafe { CStr::from_ptr(resources) }.to_string_lossy();
        text.lines()
            .find_map(|line| line.strip_prefix("Xft.dpi:").and_then(|v| v.trim().parse::<f64>().ok()))
            .map_or(1.0, |dpi| (dpi / 96.0).max(1.0))
    }

    /// The displays, as RandR's monitors (what GNOME arranges), with each one's refresh rate.
    pub fn monitors(&mut self) -> Vec<Monitor> {
        let mut count: c_int = 0;
        let infos = unsafe { (self.xrandr.XRRGetMonitors)(self.display, self.root, xlib::True, &mut count) };
        if infos.is_null() {
            return Vec::new();
        }
        let resources = unsafe { (self.xrandr.XRRGetScreenResourcesCurrent)(self.display, self.root) };
        let mut out = Vec::new();
        for i in 0..count as usize {
            let info = unsafe { &*infos.add(i) };
            let refresh = if info.noutput > 0 && !resources.is_null() { self.refresh(resources, unsafe { *info.outputs }) } else { 60 };
            out.push(Monitor {
                name: self.atom_name(info.name),
                x: info.x,
                y: info.y,
                w: info.width,
                h: info.height,
                primary: info.primary != 0,
                refresh,
            });
        }
        unsafe {
            if !resources.is_null() {
                (self.xrandr.XRRFreeScreenResources)(resources);
            }
            (self.xrandr.XRRFreeMonitors)(infos);
        }
        out
    }

    fn refresh(&self, resources: *mut x11_dl::xrandr::XRRScreenResources, output: c_ulong) -> u32 {
        unsafe {
            let info = (self.xrandr.XRRGetOutputInfo)(self.display, resources, output);
            if info.is_null() {
                return 60;
            }
            let crtc = (*info).crtc;
            (self.xrandr.XRRFreeOutputInfo)(info);
            if crtc == 0 {
                return 60;
            }
            let crtc_info = (self.xrandr.XRRGetCrtcInfo)(self.display, resources, crtc);
            if crtc_info.is_null() {
                return 60;
            }
            let mode = (*crtc_info).mode;
            (self.xrandr.XRRFreeCrtcInfo)(crtc_info);
            let r = &*resources;
            for m in 0..r.nmode as usize {
                let info = &*r.modes.add(m);
                if info.id == mode && info.hTotal > 0 && info.vTotal > 0 {
                    return (info.dotClock as f64 / (info.hTotal as f64 * info.vTotal as f64)).round() as u32;
                }
            }
            60
        }
    }

    /// Asks to be told when the displays change.
    pub fn watch_displays(&self) {
        unsafe {
            (self.xrandr.XRRSelectInput)(
                self.display,
                self.root,
                x11_dl::xrandr::RRScreenChangeNotifyMask | x11_dl::xrandr::RRCrtcChangeNotifyMask | x11_dl::xrandr::RROutputChangeNotifyMask,
            );
        }
    }

    /// Whether a 32-bit visual with alpha has this id: what a see-through window must be drawn with.
    pub fn argb_visual(&self, visual_id: i32) -> Option<(*mut xlib::Visual, c_int)> {
        let mut template: xlib::XVisualInfo = unsafe { std::mem::zeroed() };
        template.visualid = visual_id as c_ulong;
        template.screen = unsafe { (self.xlib.XDefaultScreen)(self.display) };
        let mut count = 0;
        let infos = unsafe { (self.xlib.XGetVisualInfo)(self.display, xlib::VisualIDMask | xlib::VisualScreenMask, &mut template, &mut count) };
        if infos.is_null() {
            return None;
        }
        let info = unsafe { &*infos };
        let found = (info.depth == 32).then_some((info.visual, info.depth));
        unsafe { (self.xlib.XFree)(infos.cast()) };
        found
    }

    /// A see-through overlay over everything, which takes no input at all: every click and scroll goes through.
    pub fn overlay(&mut self, visual: *mut xlib::Visual, depth: c_int, rect: (i32, i32, u32, u32)) -> Window {
        let (bypass, net_type, notification, wm_name, utf8) = (
            self.atom("_NET_WM_BYPASS_COMPOSITOR"),
            self.atom("_NET_WM_WINDOW_TYPE"),
            self.atom("_NET_WM_WINDOW_TYPE_NOTIFICATION"),
            self.atom("_NET_WM_NAME"),
            self.atom("UTF8_STRING"),
        );
        unsafe {
            let colormap = (self.xlib.XCreateColormap)(self.display, self.root, visual, xlib::AllocNone);
            let mut attributes: xlib::XSetWindowAttributes = std::mem::zeroed();
            attributes.colormap = colormap;
            attributes.border_pixel = 0;
            attributes.background_pixel = 0;
            attributes.override_redirect = xlib::True;
            let window = (self.xlib.XCreateWindow)(
                self.display,
                self.root,
                rect.0,
                rect.1,
                rect.2.max(1),
                rect.3.max(1),
                0,
                depth,
                xlib::InputOutput as u32,
                visual,
                xlib::CWColormap | xlib::CWBorderPixel | xlib::CWBackPixel | xlib::CWOverrideRedirect,
                &mut attributes,
            );
            let region = (self.xfixes.XFixesCreateRegion)(self.display, ptr::null_mut(), 0);
            (self.xfixes.XFixesSetWindowShapeRegion)(self.display, window, SHAPE_INPUT, 0, 0, region);
            (self.xfixes.XFixesDestroyRegion)(self.display, region);
            let mut class = *b"snowhush\0Snowhush\0";
            let mut hint = xlib::XClassHint { res_name: class.as_mut_ptr().cast(), res_class: class.as_mut_ptr().add(9).cast() };
            (self.xlib.XSetClassHint)(self.display, window, &mut hint);
            let name = b"Snowhush";
            (self.xlib.XChangeProperty)(self.display, window, wm_name, utf8, 8, xlib::PropModeReplace, name.as_ptr(), name.len() as c_int);
            // Never unredirected: the overlay must stay composited over what is under it.
            let never: c_ulong = 2;
            (self.xlib.XChangeProperty)(self.display, window, bypass, xlib::XA_CARDINAL, 32, xlib::PropModeReplace, (&never as *const c_ulong).cast(), 1);
            let kind = notification as c_ulong;
            (self.xlib.XChangeProperty)(self.display, window, net_type, xlib::XA_ATOM, 32, xlib::PropModeReplace, (&kind as *const c_ulong).cast(), 1);
            // Only where the input shape lets the pointer in (snowballs, the drift while Ctrl is held): presses there
            // and the drags that follow them come here.
            (self.xlib.XSelectInput)(self.display, window, xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::Button1MotionMask);
            (self.xlib.XMapRaised)(self.display, window);
            (self.xlib.XFlush)(self.display);
            window
        }
    }

    /// Where the overlay takes the pointer, in its own pixels; nowhere for none.
    pub fn set_input(&self, window: Window, rects: &[(i32, i32, u32, u32)]) {
        let mut list: Vec<xlib::XRectangle> =
            rects.iter().map(|r| xlib::XRectangle { x: r.0 as i16, y: r.1 as i16, width: r.2 as u16, height: r.3 as u16 }).collect();
        unsafe {
            let region = (self.xfixes.XFixesCreateRegion)(self.display, list.as_mut_ptr(), list.len() as c_int);
            (self.xfixes.XFixesSetWindowShapeRegion)(self.display, window, SHAPE_INPUT, 0, 0, region);
            (self.xfixes.XFixesDestroyRegion)(self.display, region);
            (self.xlib.XFlush)(self.display);
        }
    }

    pub fn move_resize(&self, window: Window, rect: (i32, i32, u32, u32)) {
        unsafe {
            (self.xlib.XMoveResizeWindow)(self.display, window, rect.0, rect.1, rect.2.max(1), rect.3.max(1));
            (self.xlib.XFlush)(self.display);
        }
    }

    pub fn raise(&self, window: Window) {
        unsafe { (self.xlib.XRaiseWindow)(self.display, window) };
    }

    pub fn destroy(&self, window: Window) {
        unsafe {
            (self.xlib.XDestroyWindow)(self.display, window);
            (self.xlib.XFlush)(self.display);
        }
    }

    /// Takes every event waiting, without blocking: whether the displays changed, and the pointer on the overlays
    /// (of a run of drags, only the last).
    pub fn drain(&self, randr_base: c_int) -> (bool, Vec<Pointer>) {
        let mut changed = false;
        let mut pointer: Vec<Pointer> = Vec::new();
        unsafe {
            while (self.xlib.XPending)(self.display) > 0 {
                let mut event: xlib::XEvent = std::mem::zeroed();
                (self.xlib.XNextEvent)(self.display, &mut event);
                let kind = event.get_type();
                if kind >= randr_base && kind <= randr_base + x11_dl::xrandr::RRNotify {
                    (self.xrandr.XRRUpdateConfiguration)(&mut event);
                    changed = true;
                    continue;
                }
                let next = match kind {
                    xlib::ButtonPress | xlib::ButtonRelease if event.button.button == xlib::Button1 => {
                        let b = event.button;
                        let kind = if kind == xlib::ButtonPress { PointerKind::Press } else { PointerKind::Release };
                        Pointer { window: b.window, kind, x: b.x_root, y: b.y_root, state: b.state }
                    }
                    xlib::MotionNotify => {
                        let m = event.motion;
                        Pointer { window: m.window, kind: PointerKind::Drag, x: m.x_root, y: m.y_root, state: m.state }
                    }
                    _ => continue,
                };
                if next.kind == PointerKind::Drag
                    && let Some(last) = pointer.last_mut()
                    && last.kind == PointerKind::Drag
                {
                    *last = next;
                } else {
                    pointer.push(next);
                }
            }
        }
        (changed, pointer)
    }

    pub fn randr_event_base(&self) -> c_int {
        let (mut event, mut error) = (0, 0);
        unsafe { (self.xrandr.XRRQueryExtension)(self.display, &mut event, &mut error) };
        event
    }

    /// A 32-bit property as numbers (windows, atoms, cardinals), or nothing.
    pub fn cardinals(&mut self, window: Window, name: &str) -> Option<Vec<u64>> {
        let property = self.atom(name);
        self.property(window, property).map(|(data, format, n)| {
            if format != 32 {
                return Vec::new();
            }
            // Xlib hands 32-bit items to a 64-bit client as longs.
            let items = data.as_ptr() as *const c_long;
            (0..n).map(|i| unsafe { *items.add(i) } as u64 & 0xffff_ffff).collect()
        })
    }

    /// An 8-bit property as text (WM_CLASS keeps two strings, split at the NUL).
    pub fn text(&mut self, window: Window, name: &str) -> Option<Vec<String>> {
        let property = self.atom(name);
        self.property(window, property).filter(|(_, format, _)| *format == 8).map(|(data, _, n)| {
            data[..n].split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect()
        })
    }

    fn property(&self, window: Window, property: xlib::Atom) -> Option<(Vec<u8>, c_int, usize)> {
        let (mut kind, mut format, mut n, mut after): (xlib::Atom, c_int, c_ulong, c_ulong) = (0, 0, 0, 0);
        let mut data: *mut c_uchar = ptr::null_mut();
        let status = unsafe {
            (self.xlib.XGetWindowProperty)(
                self.display,
                window,
                property,
                0,
                4096,
                xlib::False,
                xlib::AnyPropertyType as c_ulong,
                &mut kind,
                &mut format,
                &mut n,
                &mut after,
                &mut data,
            )
        };
        if status != xlib::Success as c_int || data.is_null() {
            return None;
        }
        let item = match format {
            8 => 1,
            16 => std::mem::size_of::<std::ffi::c_short>(),
            32 => std::mem::size_of::<c_long>(),
            _ => 0,
        };
        let bytes = unsafe { std::slice::from_raw_parts(data, n as usize * item) }.to_vec();
        unsafe { (self.xlib.XFree)(data.cast::<c_void>()) };
        if kind == 0 { None } else { Some((bytes, format, n as usize)) }
    }

    /// A window's inside, in root coordinates, and whether it is mapped where it can show.
    pub fn geometry(&self, window: Window) -> Option<(i32, i32, u32, u32, bool)> {
        unsafe {
            let mut attributes: xlib::XWindowAttributes = std::mem::zeroed();
            if (self.xlib.XGetWindowAttributes)(self.display, window, &mut attributes) == 0 {
                return None;
            }
            let (mut x, mut y, mut child) = (0, 0, 0);
            (self.xlib.XTranslateCoordinates)(self.display, window, self.root, 0, 0, &mut x, &mut y, &mut child);
            Some((x, y, attributes.width as u32, attributes.height as u32, attributes.map_state == xlib::IsViewable))
        }
    }

    /// The pointer in root coordinates, whether a mouse button is held, and the modifier keys and buttons held.
    pub fn pointer(&self) -> Option<(i32, i32, bool, u32)> {
        unsafe {
            let (mut root, mut child, mut rx, mut ry, mut wx, mut wy, mut mask) = (0, 0, 0, 0, 0, 0, 0);
            let ok = (self.xlib.XQueryPointer)(self.display, self.root, &mut root, &mut child, &mut rx, &mut ry, &mut wx, &mut wy, &mut mask);
            (ok != 0).then_some((rx, ry, mask & (xlib::Button1Mask | xlib::Button2Mask | xlib::Button3Mask) != 0, mask))
        }
    }

    /// Asks for property changes on the root (the window list, the desktop) and structure changes of its
    /// children (window frames moving, mapping, going).
    pub fn watch_root(&self) {
        unsafe {
            (self.xlib.XSelectInput)(self.display, self.root, xlib::PropertyChangeMask | xlib::SubstructureNotifyMask);
        }
    }

    pub fn watch_window(&self, window: Window) {
        unsafe {
            (self.xlib.XSelectInput)(self.display, window, xlib::PropertyChangeMask | xlib::StructureNotifyMask);
        }
    }

    /// Waits up to `timeout` for the server to send something; returns whether it did.
    pub fn wait(&self, timeout: std::time::Duration) -> bool {
        if unsafe { (self.xlib.XPending)(self.display) } > 0 {
            return true;
        }
        let mut fd = libc::pollfd { fd: self.connection_fd(), events: libc::POLLIN, revents: 0 };
        let ms = timeout.as_millis().min(i32::MAX as u128) as c_int;
        unsafe { libc::poll(&mut fd, 1, ms) > 0 }
    }

    /// Takes every waiting event and returns how many there were; the window source only wants to know that
    /// something changed.
    pub fn take_events(&self) -> usize {
        let mut n = 0;
        unsafe {
            while (self.xlib.XPending)(self.display) > 0 {
                let mut event: xlib::XEvent = std::mem::zeroed();
                (self.xlib.XNextEvent)(self.display, &mut event);
                // Our own overlays moving are no news.
                if event.get_type() == xlib::ConfigureNotify && event.configure.override_redirect != 0 {
                    continue;
                }
                n += 1;
            }
        }
        n
    }

    /// Ignores the errors a window going away between two requests causes, instead of Xlib's default handler
    /// ending the process.
    pub fn quiet_errors(&self) {
        unsafe extern "C" fn ignore(_: *mut xlib::Display, _: *mut xlib::XErrorEvent) -> c_int {
            0
        }
        unsafe { (self.xlib.XSetErrorHandler)(Some(ignore)) };
    }
}

impl Drop for X {
    fn drop(&mut self) {
        unsafe { (self.xlib.XCloseDisplay)(self.display) };
    }
}
