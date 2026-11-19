//! What the X11 port keeps of linux/src/kwin.rs: the D-Bus name that lets only one Snowhush run, and the type of a
//! desktop switch sliding sideways, which only KWin animates and so never happens here.

use std::sync::Arc;
use std::time::Instant;

use zbus::blocking::Connection;

use crate::source::Shared;

pub const SERVICE: &str = "io.github.mbezruchko.Snowhush";

/// The session bus with Snowhush's name taken, or None when another copy has it.
pub fn serve(_shared: Arc<Shared>) -> zbus::Result<Option<Connection>> {
    let built = zbus::blocking::connection::Builder::session()?
        .name(SERVICE)?
        .allow_name_replacements(false)
        .replace_existing_names(false)
        .build();
    match built {
        Ok(connection) => Ok(Some(connection)),
        Err(zbus::Error::NameTaken) => Ok(None),
        Err(error) => Err(error),
    }
}

/// A desktop switch sliding the windows sideways (KWin only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slide {
    start: Instant,
}

impl Slide {
    pub fn velocity(&self, _now: Instant) -> f32 {
        let _ = self.start;
        0.0
    }
}
