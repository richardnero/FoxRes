//! Renders the window to PNG files for the README and store listing.
//!
//! Set `FOXRES_SCREENSHOTS=/some/dir` and start the app with no arguments.
//! It fills each page with sample input, saves a PNG of each, and quits.

use std::path::{Path, PathBuf};

use relm4::{adw::prelude::*, gtk};

/// Output directory, if screenshot mode was requested.
pub fn dir() -> Option<PathBuf> {
    std::env::var_os("FOXRES_SCREENSHOTS").map(PathBuf::from)
}

/// One screenshot: which page, what to type, and the file name.
pub struct Shot {
    pub page: &'static str,
    pub file: &'static str,
    pub dark: bool,
}

pub const SHOTS: [Shot; 5] = [
    Shot {
        page: "bands",
        file: "color-code.png",
        dark: false,
    },
    Shot {
        page: "smd",
        file: "smd.png",
        dark: false,
    },
    Shot {
        page: "eseries",
        file: "e-series.png",
        dark: false,
    },
    Shot {
        page: "combine",
        file: "combine.png",
        dark: false,
    },
    Shot {
        page: "bands",
        file: "color-code-dark.png",
        dark: true,
    },
];

/// Save `widget` as a PNG at its current size.
pub fn save(widget: &impl IsA<gtk::Widget>, path: &Path) -> Result<(), String> {
    let widget = widget.as_ref();
    let (w, h) = (f64::from(widget.width()), f64::from(widget.height()));
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, w, h);
    let node = snapshot.to_node().ok_or("nothing was drawn")?;
    let renderer = widget
        .native()
        .and_then(|n| n.renderer())
        .ok_or("window has no renderer")?;
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).map_err(|e| e.to_string())
}
