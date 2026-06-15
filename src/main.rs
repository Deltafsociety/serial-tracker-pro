//! STPRO — Series Tracker Pro.
//!
//! Reads your recently-used files, flags everything matching the `S00E00`
//! pattern (case-insensitive), and turns it into a "continue watching" board:
//! which series you're on, the last episode you watched, and what's next.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod library;

use eframe::egui;

fn main() -> eframe::Result<()> {
    // Headless mode: `stpro --list` prints the flagged series to stdout. Handy on
    // machines with no display server, and how the scan logic is verified.
    if std::env::args().any(|a| a == "--list") {
        let scan = library::scan_recent();
        println!(
            "STPRO — scanned {} recent files, flagged {} series\n",
            scan.total_files_seen,
            scan.series.len()
        );
        for s in &scan.series {
            let avail = if s.next_available() { "available" } else { "missing" };
            println!(
                "▶ {}  ({} seasons, {} eps)\n    last watched: {}  ({})\n    watch next:   {}  [{}]\n",
                s.name,
                s.seasons(),
                s.watched_count(),
                s.last_watched.tag(),
                s.last_watched_date(),
                s.next_up_tag(),
                avail,
            );
        }
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 760.0])
        .with_min_inner_size([720.0, 520.0])
        .with_title("STPRO — Series Tracker Pro");
    if std::env::var("STPRO_MAX").is_ok() {
        viewport = viewport.with_maximized(true);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "STPRO",
        options,
        Box::new(|cc| Ok(Box::new(app::StproApp::new(cc)))),
    )
}
