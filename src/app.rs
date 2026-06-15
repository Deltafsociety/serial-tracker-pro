//! STPRO GUI — a dark, card-based "continue watching" dashboard built on egui.

use eframe::egui;
use egui::{Align, Color32, FontId, Layout, Margin, RichText, Rounding, Stroke, Vec2};

use crate::library::{self, Series};

// ---- palette ---------------------------------------------------------------
const BG: Color32 = Color32::from_rgb(0x0f, 0x11, 0x17);
const PANEL: Color32 = Color32::from_rgb(0x15, 0x18, 0x22);
const CARD: Color32 = Color32::from_rgb(0x1b, 0x1f, 0x2b);
const CARD_HOVER: Color32 = Color32::from_rgb(0x22, 0x27, 0x36);
const BORDER: Color32 = Color32::from_rgb(0x2a, 0x30, 0x42);
const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe9, 0xf2);
const MUTED: Color32 = Color32::from_rgb(0x8a, 0x91, 0xa6);
const GREEN: Color32 = Color32::from_rgb(0x3f, 0xd1, 0x8b);
const AMBER: Color32 = Color32::from_rgb(0xf2, 0xb1, 0x4c);

fn accent() -> Color32 {
    Color32::from_rgb(0x6c, 0x6c, 0xff)
}

pub struct StproApp {
    series: Vec<Series>,
    source: String,
    files_seen: usize,
    search: String,
    status: String,
    selected: Option<usize>,
}

impl StproApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        let mut app = Self {
            series: Vec::new(),
            source: String::new(),
            files_seen: 0,
            search: String::new(),
            status: String::new(),
            selected: None,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        let scan = library::scan_recent();
        self.files_seen = scan.total_files_seen;
        self.source = scan.source;
        self.series = scan.series;
        self.status = format!(
            "Scanned {} recent files → {} series flagged",
            self.files_seen,
            self.series.len()
        );
        self.selected = None;
    }

    fn add_folder(&mut self, dir: &str) {
        let scan = library::scan_dir_full(dir);
        let added = scan.series.len();
        let existing = std::mem::take(&mut self.series);
        self.series = library::merge(existing, scan.series);
        self.status = format!("Added {} (+{} series) → {} total", dir, added, self.series.len());
    }

    fn total_episodes(&self) -> usize {
        self.series.iter().map(|s| s.watched_count()).sum()
    }
}

impl eframe::App for StproApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        top_bar(self, ctx);
        bottom_bar(self, ctx);
        central(self, ctx);
        detail_window(self, ctx);
    }
}

fn top_bar(app: &mut StproApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top")
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::symmetric(20.0, 14.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Logo mark.
                logo(ui);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("STPRO").size(22.0).strong().color(TEXT));
                    ui.label(
                        RichText::new("Series Tracker Pro")
                            .size(12.0)
                            .color(MUTED),
                    );
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill_button(ui, "⟳  Rescan", accent()).clicked() {
                        app.refresh();
                    }
                    ui.add_space(8.0);
                    if pill_button(ui, "＋  Add folder", CARD).clicked() {
                        // No native dialog dependency: scan common video dirs.
                        let home = std::env::var("HOME").unwrap_or_default();
                        for sub in ["Videos", "Downloads", "Movies"] {
                            let dir = format!("{home}/{sub}");
                            if std::path::Path::new(&dir).is_dir() {
                                app.add_folder(&dir);
                            }
                        }
                    }
                    ui.add_space(16.0);
                    // Search box.
                    ui.add_sized(
                        Vec2::new(220.0, 30.0),
                        egui::TextEdit::singleline(&mut app.search)
                            .hint_text("🔍  Search series")
                            .vertical_align(Align::Center),
                    );
                });
            });
        });
}

fn bottom_bar(app: &StproApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("status")
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(Margin::symmetric(20.0, 8.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&app.status).size(12.0).color(MUTED));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("source: {}", app.source))
                            .size(11.0)
                            .color(Color32::from_rgb(0x55, 0x5c, 0x70)),
                    );
                });
            });
        });
}

fn central(app: &mut StproApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(BG).inner_margin(Margin::same(20.0)))
        .show(ctx, |ui| {
            stats_row(app, ui);
            ui.add_space(18.0);

            if app.series.is_empty() {
                empty_state(ui);
                return;
            }

            ui.label(
                RichText::new("CONTINUE WATCHING")
                    .size(13.0)
                    .strong()
                    .color(MUTED),
            );
            ui.add_space(10.0);

            let query = app.search.to_lowercase();
            let indices: Vec<usize> = app
                .series
                .iter()
                .enumerate()
                .filter(|(_, s)| query.is_empty() || s.name.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let avail = ui.available_width();
                    let card_w = 320.0_f32;
                    let gap = 16.0;
                    let cols = ((avail + gap) / (card_w + gap)).floor().max(1.0) as usize;

                    let mut clicked: Option<usize> = None;
                    for row in indices.chunks(cols) {
                        ui.horizontal(|ui| {
                            for &i in row {
                                if series_card(ui, &app.series[i], card_w).clicked() {
                                    clicked = Some(i);
                                }
                                ui.add_space(gap);
                            }
                        });
                        ui.add_space(gap);
                    }
                    if clicked.is_some() {
                        app.selected = clicked;
                    }
                });
        });
}

fn stats_row(app: &StproApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        stat_card(ui, "Series", &app.series.len().to_string(), accent());
        ui.add_space(14.0);
        stat_card(ui, "Episodes watched", &app.total_episodes().to_string(), GREEN);
        ui.add_space(14.0);
        let cont = app
            .series
            .iter()
            .filter(|s| s.next_available())
            .count()
            .to_string();
        stat_card(ui, "Ready to continue", &cont, AMBER);
    });
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, accent_color: Color32) {
    egui::Frame::none()
        .fill(PANEL)
        .rounding(Rounding::same(14.0))
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(Margin::symmetric(20.0, 14.0))
        .show(ui, |ui| {
            ui.set_width(190.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, accent_color);
                    ui.add_space(2.0);
                    ui.label(RichText::new(label).size(12.0).color(MUTED));
                });
                ui.label(RichText::new(value).size(30.0).strong().color(TEXT));
            });
        });
}

/// Fixed card height so every card in a row lines up.
const CARD_H: f32 = 168.0;

/// Render one series card in a fixed-size cell. Returns the click response.
fn series_card(ui: &mut egui::Ui, s: &Series, width: f32) -> egui::Response {
    let color = series_color(&s.name);
    let inner_w = width - 32.0;

    // Allocate a fixed-width cell with an explicit top-down layout. Fixing the
    // width prevents egui's "one char per line" wrapping; forcing top-down stops
    // the inner rows from inheriting the parent row's horizontal layout.
    let cell = ui.allocate_ui_with_layout(
        Vec2::new(width, CARD_H),
        Layout::top_down(Align::Min),
        |ui| {
            ui.set_min_size(Vec2::new(width, CARD_H));
        egui::Frame::none()
            .fill(CARD)
            .rounding(Rounding::same(16.0))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(Margin::same(16.0))
            .show(ui, |ui| {
                ui.set_width(inner_w);
                ui.set_min_height(CARD_H - 32.0);

                // Header: avatar + title.
                ui.horizontal(|ui| {
                    avatar(ui, &s.name, color);
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.set_width(inner_w - 58.0);
                        no_wrap(ui, &s.name, 16.0, TEXT, true);
                        no_wrap(
                            ui,
                            &format!("{} seasons · {} watched", s.seasons(), s.watched_count()),
                            11.5,
                            MUTED,
                            false,
                        );
                    });
                });

                ui.add_space(12.0);

                // Last watched / Watch next.
                ui.horizontal(|ui| {
                    tag_block(ui, "LAST WATCHED", &s.last_watched.tag(), MUTED, TEXT);
                    ui.add_space(10.0);
                    let nc = if s.next_available() { GREEN } else { AMBER };
                    tag_block(ui, "WATCH NEXT", &s.next_up_tag(), nc, nc);
                });

                ui.add_space(12.0);

                // Season progress.
                progress(ui, s.season_progress(), color, inner_w);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    no_wrap(
                        ui,
                        &format!("Season {}", s.last_watched.season),
                        11.0,
                        MUTED,
                        false,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let note = if s.next_available() {
                            format!("seen {}", s.last_watched_date())
                        } else {
                            format!("⤓ {} not downloaded", s.next_up_tag())
                        };
                        let c = if s.next_available() { MUTED } else { AMBER };
                        no_wrap(ui, &note, 11.0, c, false);
                    });
                });
            });
    });

    let rect = cell.response.rect;
    let resp = ui.interact(
        rect,
        ui.make_persistent_id(("card", &s.name)),
        egui::Sense::click(),
    );
    if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            Rounding::same(16.0),
            Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
        ui.painter()
            .rect_stroke(rect, Rounding::same(16.0), Stroke::new(1.5, color));
    }
    resp
}

/// A label that never wraps (extends to its natural width / truncates).
fn no_wrap(ui: &mut egui::Ui, text: &str, size: f32, color: Color32, strong: bool) {
    let mut rt = RichText::new(text).size(size).color(color);
    if strong {
        rt = rt.strong();
    }
    ui.add(egui::Label::new(rt).truncate());
}

fn detail_window(app: &mut StproApp, ctx: &egui::Context) {
    let Some(idx) = app.selected else {
        return;
    };
    if idx >= app.series.len() {
        app.selected = None;
        return;
    }
    let s = app.series[idx].clone();
    let mut open = true;
    egui::Window::new(RichText::new(&s.name).size(18.0).strong().color(TEXT))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(520.0, 480.0))
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .rounding(Rounding::same(14.0))
                .stroke(Stroke::new(1.0, BORDER))
                .inner_margin(Margin::same(18.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                tag_block(ui, "LAST WATCHED", &s.last_watched.tag(), MUTED, TEXT);
                ui.add_space(10.0);
                let nc = if s.next_available() { GREEN } else { AMBER };
                tag_block(ui, "WATCH NEXT", &s.next_up_tag(), nc, nc);
                ui.add_space(10.0);
                tag_block(
                    ui,
                    "WATCHED",
                    &format!("{} eps", s.watched_count()),
                    accent(),
                    TEXT,
                );
            });
            ui.add_space(14.0);
            ui.label(RichText::new("ALL FLAGGED EPISODES").size(12.0).strong().color(MUTED));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                let last = s.last_watched.tag();
                for ep in &s.episodes {
                    let is_last = ep.tag() == last;
                    egui::Frame::none()
                        .fill(if is_last { CARD_HOVER } else { CARD })
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width() - 4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(ep.tag())
                                        .size(13.0)
                                        .strong()
                                        .color(if is_last { GREEN } else { TEXT }),
                                );
                                ui.add_space(10.0);
                                ui.label(
                                    RichText::new(&ep.filename)
                                        .size(11.0)
                                        .color(MUTED),
                                )
                                .on_hover_text(&ep.path);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(
                                            ep.visited.split('T').next().unwrap_or(""),
                                        )
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
            });
        });
    if !open {
        app.selected = None;
    }
}

// ---- small widgets ---------------------------------------------------------

fn tag_block(ui: &mut egui::Ui, label: &str, value: &str, label_c: Color32, value_c: Color32) {
    egui::Frame::none()
        .fill(PANEL)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(label).size(9.5).color(label_c)).extend(),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(value).size(15.0).strong().color(value_c).monospace(),
                    )
                    .extend(),
                );
            });
        });
}

fn progress(ui: &mut egui::Ui, frac: f32, color: Color32, width: f32) {
    let h = 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(0x26, 0x2c, 0x3c));
    let mut fill = rect;
    fill.set_width((rect.width() * frac).max(if frac > 0.0 { 6.0 } else { 0.0 }));
    painter.rect_filled(fill, Rounding::same(4.0), color);
}

fn avatar(ui: &mut egui::Ui, name: &str, color: Color32) {
    let size = 46.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(12.0), color.gamma_multiply(0.28));
    painter.rect_stroke(rect, Rounding::same(12.0), Stroke::new(1.5, color));
    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        FontId::proportional(18.0),
        color,
    );
}

fn logo(ui: &mut egui::Ui) {
    let size = 40.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(11.0), accent());
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "▶",
        FontId::proportional(18.0),
        Color32::WHITE,
    );
}

fn pill_button(ui: &mut egui::Ui, text: &str, fill: Color32) -> egui::Response {
    let resp = ui
        .add_sized(
            Vec2::new(0.0, 30.0),
            egui::Button::new(RichText::new(text).size(13.0).color(TEXT))
                .fill(fill)
                .rounding(Rounding::same(9.0))
                .stroke(Stroke::new(1.0, BORDER)),
        );
    resp
}

fn empty_state(ui: &mut egui::Ui) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("📺").size(48.0));
        ui.add_space(8.0);
        ui.label(
            RichText::new("No series flagged yet")
                .size(18.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Open some episodes named like Show.S01E01.mkv, then hit Rescan — or Add folder.",
            )
            .size(13.0)
            .color(MUTED),
        );
    });
}

// ---- helpers ---------------------------------------------------------------

/// Deterministic vivid color from a series name (golden-ratio hue hashing).
fn series_color(name: &str) -> Color32 {
    let mut h: u64 = 1469598103934665603; // FNV offset
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    let hue = (h % 360) as f32;
    hsv_to_rgb(hue, 0.55, 0.95)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;
    v.dark_mode = true;
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(0x10, 0x13, 0x1c);
    v.widgets.inactive.bg_fill = CARD;
    v.widgets.inactive.weak_bg_fill = CARD;
    v.widgets.hovered.bg_fill = CARD_HOVER;
    v.widgets.hovered.weak_bg_fill = CARD_HOVER;
    v.widgets.active.bg_fill = CARD_HOVER;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.selection.bg_fill = accent().gamma_multiply(0.4);
    v.window_rounding = Rounding::same(14.0);
    v.window_stroke = Stroke::new(1.0, BORDER);

    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    ctx.set_style(style);
}
