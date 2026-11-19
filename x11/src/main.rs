//! Snowhush on X11 (GNOME on Xorg): linux/'s app with its Wayland parts (layer surfaces, compositor IPC) swapped for
//! X11 overlay windows and the window manager's EWMH properties. The modules that don't touch Wayland are linux/'s
//! own files, used as they are.

#[path = "../../linux/src/chores.rs"]
mod chores;
mod kwin;
#[path = "../../linux/src/light.rs"]
mod light;
#[path = "../../linux/src/log.rs"]
mod log;
#[path = "../../linux/src/prefs.rs"]
#[allow(dead_code)]
mod prefs;
mod render;
#[allow(dead_code)]
mod screen;
mod snow;
#[allow(dead_code)]
mod source;
mod tray;
#[path = "../../linux/src/windows.rs"]
mod windows;
#[allow(dead_code)]
mod xlib;
mod xsource;

fn main() {
    snowhush_core::text::choose(&languages());
    if let Err(error) = snow::run() {
        log!("failed: {error}");
        eprintln!("Snowhush couldn't start: {error}");
        std::process::exit(1);
    }
}

/// The user's languages as gettext ranks them: `LANGUAGE`'s list, then `LC_ALL`, `LC_MESSAGES` and `LANG`.
fn languages() -> Vec<String> {
    let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let mut tags: Vec<String> = var("LANGUAGE").map(|list| list.split(':').map(str::to_string).collect()).unwrap_or_default();
    tags.extend(["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().filter_map(var));
    tags
}
