//! Professional performance dashboard (Task Manager–style graphs + metric blocks).
//!
//! Layout rules:
//! - Every panel is given an *exact* pixel rect and plots fill that rect.
//! - History is right-aligned into a fixed window (newest on the right) so
//!   early samples don't leave a huge empty right side.

use crate::fold::{
    BRIDGE_VERSION, EXCURSION_BAND, FOLD_ID, INSTRUMENT_VERSION, INTEROCEPTION_PORT, PIN_TAG,
    PLANT_D_EFF, PLANT_N, PLANT_P,
};
use crate::monitor::{Engine, HISTORY_LEN};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Frame, Id, Layout, Margin, RichText, Sense, Stroke,
    Ui, Vec2, Vec2b,
};
use egui_plot::{Bar, BarChart, GridMark, Line, Plot, PlotPoints};
use std::collections::VecDeque;

// --- palette ---
const BG: Color32 = Color32::from_rgb(18, 20, 26);
const PANEL: Color32 = Color32::from_rgb(28, 32, 42);
const PANEL_HI: Color32 = Color32::from_rgb(36, 42, 56);
const BORDER: Color32 = Color32::from_rgb(52, 60, 78);
const TEXT: Color32 = Color32::from_rgb(230, 234, 242);
const MUTED: Color32 = Color32::from_rgb(140, 150, 170);
const CPU_C: Color32 = Color32::from_rgb(80, 160, 255);
const MEM_C: Color32 = Color32::from_rgb(80, 210, 140);
const GPU_C: Color32 = Color32::from_rgb(180, 120, 255);
const TEMP_C: Color32 = Color32::from_rgb(255, 140, 80);
const S_C: Color32 = Color32::from_rgb(90, 220, 220);
const T1_C: Color32 = Color32::from_rgb(100, 180, 255);
const T2_C: Color32 = Color32::from_rgb(255, 200, 90);
const T3_C: Color32 = Color32::from_rgb(255, 120, 160);
const NEG_C: Color32 = Color32::from_rgb(90, 140, 255);
const ZERO_C: Color32 = Color32::from_rgb(120, 130, 150);
const POS_C: Color32 = Color32::from_rgb(255, 110, 110);
const GOOD: Color32 = Color32::from_rgb(70, 200, 120);
const WARN: Color32 = Color32::from_rgb(240, 180, 60);
const HOT: Color32 = Color32::from_rgb(240, 80, 80);
const ALERT_BG: Color32 = Color32::from_rgb(60, 22, 28);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPanel {
    Overview,
    Cpu,
    Mem,
    Gpu,
    Temp,
    Fsot,
    Science,
}

pub struct MonitorApp {
    pub engine: Engine,
    always_on_top: bool,
    compact: bool,
    focus: FocusPanel,
    /// Flash timer (frames remaining) when a POS alert fires.
    alert_flash: u32,
    last_alert_key: u64,
}

impl MonitorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let mut engine = Engine::new();
        engine.sample_now();
        Self {
            engine,
            always_on_top: false,
            compact: false,
            focus: FocusPanel::Overview,
            alert_flash: 0,
            last_alert_key: 0,
        }
    }

    fn update_alerts(&mut self) {
        if self.alert_flash > 0 {
            self.alert_flash -= 1;
        }
        let Some(s) = self.engine.latest.as_ref() else {
            return;
        };
        let hot_temp = s.temp_c.map(|t| t >= 85.0).unwrap_or(false);
        let hot_cpu = s.cpu_pct >= 90.0;
        let hot_gpu = s.gpu_pct >= 95.0;
        let s_excursion = (s.fsot.scalar_s - s.fsot.s_ref).abs() > EXCURSION_BAND;
        let pos_temp = s.fsot.trit_temp > 0;

        if hot_temp || hot_cpu || hot_gpu || s_excursion || pos_temp {
            let key = (s.cpu_pct as u64)
                ^ ((s.gpu_pct as u64) << 8)
                ^ (s.temp_c.unwrap_or(0.0) as u64) << 16
                ^ ((s.fsot.trit_s as u64) << 24);
            if key != self.last_alert_key {
                self.last_alert_key = key;
                self.alert_flash = 24;
            }
        }
    }
}

impl eframe::App for MonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.engine.maybe_sample();
        self.update_alerts();
        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        let mut want_on_top: Option<bool> = None;
        let mut want_compact: Option<bool> = None;
        let mut want_export = false;
        let mut want_stream: Option<bool> = None;
        let alert_flash = self.alert_flash;

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::symmetric(10, 8)))
            .show(ctx, |ui| {
                // ---- toolbar row ----
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if self.compact {
                            "FSOT Monitor"
                        } else {
                            "FSOT PC Monitor · Scientific plant"
                        })
                        .color(TEXT)
                        .size(if self.compact { 14.0 } else { 17.0 })
                        .strong(),
                    );

                    ui.add_space(8.0);
                    for (label, panel) in [
                        ("Overview", FocusPanel::Overview),
                        ("CPU", FocusPanel::Cpu),
                        ("Mem", FocusPanel::Mem),
                        ("GPU", FocusPanel::Gpu),
                        ("Temp", FocusPanel::Temp),
                        ("FSOT", FocusPanel::Fsot),
                        ("Science", FocusPanel::Science),
                    ] {
                        if !self.compact
                            && ui
                                .selectable_label(self.focus == panel, label)
                                .on_hover_text("Focus this panel")
                                .clicked()
                        {
                            self.focus = panel;
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Export run").color(TEXT))
                            .on_hover_text("Write certificate.json + telemetry + science.csv")
                            .clicked()
                        {
                            want_export = true;
                        }
                        if ui
                            .selectable_label(self.engine.stream.enabled(), "Neuron stream")
                            .on_hover_text(format!(
                                "UDP interoception → 127.0.0.1:{INTEROCEPTION_PORT}"
                            ))
                            .clicked()
                        {
                            want_stream = Some(!self.engine.stream.enabled());
                        }
                        if ui
                            .selectable_label(self.always_on_top, "📌 On top")
                            .on_hover_text("Keep window above others")
                            .clicked()
                        {
                            want_on_top = Some(!self.always_on_top);
                        }
                        if ui
                            .selectable_label(self.compact, "▣ Compact")
                            .on_hover_text("Small always-handy strip")
                            .clicked()
                        {
                            want_compact = Some(!self.compact);
                        }
                    });
                });

                // Session certificate strip
                if !self.compact {
                    certificate_strip(ui, &self.engine);
                }

                // Alert banner
                if let Some(msg) = alert_message(&self.engine, alert_flash > 0) {
                    let bg = if alert_flash > 0 { ALERT_BG } else { PANEL };
                    Frame::new()
                        .fill(bg)
                        .stroke(Stroke::new(1.0, HOT))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(RichText::new(msg).color(HOT).strong().size(13.0));
                        });
                    ui.add_space(4.0);
                }

                if let Some(msg) = self.engine.last_export_msg.as_ref() {
                    ui.label(RichText::new(msg).color(GOOD).size(11.0));
                }

                if let Some(s) = self.engine.latest.as_ref() {
                    ui.label(
                        RichText::new(format!(
                            "V0 CPU {:.1}% · RAM {:.1} GB · temp {} · load {:.3} · sample {}",
                            self.engine.v0.cpu_usage,
                            self.engine.v0.mem_used_gb,
                            self.engine
                                .v0
                                .temp_c
                                .map(|t| format!("{:.0}°C", t))
                                .unwrap_or_else(|| "n/a".into()),
                            s.fsot.load,
                            s.timestamp
                        ))
                        .color(MUTED)
                        .size(11.0),
                    );
                }

                ui.add_space(6.0);

                // ---- metric cards (always) ----
                let avail_w = ui.available_width();
                let card_h = if self.compact { 78.0 } else { 88.0 };
                let n_cards = 5.0;
                let gap = 6.0;
                let card_w = ((avail_w - gap * (n_cards - 1.0)) / n_cards).max(100.0);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    if let Some(s) = self.engine.latest.as_ref() {
                        let cards = [
                            (
                                "CPU",
                                format!("{:.0}%", s.cpu_pct),
                                self.engine.cpu_brand.clone(),
                                (s.cpu_pct as f64 / 100.0).clamp(0.0, 1.0),
                                CPU_C,
                                load_color(s.cpu_pct as f64),
                                FocusPanel::Cpu,
                            ),
                            (
                                "MEMORY",
                                format!("{:.0}%", s.mem_frac * 100.0),
                                format!("{:.1} / {:.1} GB", s.mem_used_gb, s.mem_total_gb),
                                s.mem_frac,
                                MEM_C,
                                load_color(s.mem_frac * 100.0),
                                FocusPanel::Mem,
                            ),
                            (
                                "GPU",
                                format!("{:.0}%", s.gpu_pct),
                                if s.gpu_vram_mb > 0 {
                                    format!("{} · {} MB", short_name(&s.gpu_name, 16), s.gpu_vram_mb)
                                } else {
                                    short_name(&s.gpu_name, 24)
                                },
                                (s.gpu_pct as f64 / 100.0).clamp(0.0, 1.0),
                                GPU_C,
                                load_color(s.gpu_pct as f64),
                                FocusPanel::Gpu,
                            ),
                            (
                                "TEMP",
                                s.temp_c
                                    .map(|t| format!("{:.0}°C", t))
                                    .unwrap_or_else(|| "n/a".into()),
                                s.temp_c
                                    .map(|_| "primary sensor".into())
                                    .unwrap_or_else(|| "no sensor".into()),
                                s.temp_c
                                    .map(|t| ((t - 20.0) / 80.0).clamp(0.0, 1.0))
                                    .unwrap_or(0.0),
                                TEMP_C,
                                s.temp_c.map(temp_color).unwrap_or(MUTED),
                                FocusPanel::Temp,
                            ),
                            (
                                "FSOT S",
                                format!("{:+.3}", s.fsot.scalar_s),
                                format!("S_ref {:+.3} · {FOLD_ID}", s.fsot.s_ref),
                                ((s.fsot.scalar_s + 3.0) / 6.0).clamp(0.0, 1.0),
                                S_C,
                                s_color(s.fsot.scalar_s),
                                FocusPanel::Fsot,
                            ),
                        ];

                        for (title, value, sub, frac, accent, vcol, panel) in cards {
                            if metric_card(ui, card_w, card_h, title, &value, &sub, frac, accent, vcol)
                                && !self.compact
                            {
                                self.focus = if self.focus == panel {
                                    FocusPanel::Overview
                                } else {
                                    panel
                                };
                            }
                        }
                    } else {
                        ui.label(RichText::new("Sampling…").color(MUTED));
                    }
                });

                if self.compact {
                    return; // strip mode: cards only
                }

                ui.add_space(8.0);

                let body_h = ui.available_height().max(200.0);
                let hist = &self.engine.history;
                let (tmin, tmax) = temp_axis_range(&hist.temp);

                match self.focus {
                    FocusPanel::Overview => {
                        // 2×2 graphs take ~58% height; bottom row the rest
                        let gap = 6.0;
                        let top_h = (body_h * 0.52).clamp(160.0, body_h - 160.0);
                        let bot_h = (body_h - top_h - gap).max(140.0);
                        let half_w = (avail_w - gap) / 2.0;
                        let cell_h = (top_h - gap) / 2.0;

                        // Row 1
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            graph_cell(
                                ui,
                                half_w,
                                cell_h,
                                "cpu",
                                "CPU utilization",
                                "%",
                                0.0,
                                100.0,
                                CPU_C,
                                &hist.cpu,
                                true,
                            );
                            graph_cell(
                                ui,
                                half_w,
                                cell_h,
                                "mem",
                                "Memory",
                                "%",
                                0.0,
                                100.0,
                                MEM_C,
                                &hist.mem,
                                true,
                            );
                        });
                        ui.add_space(gap);
                        // Row 2
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            graph_cell(
                                ui,
                                half_w,
                                cell_h,
                                "gpu",
                                "GPU utilization",
                                "%",
                                0.0,
                                100.0,
                                GPU_C,
                                &hist.gpu,
                                true,
                            );
                            graph_cell(
                                ui,
                                half_w,
                                cell_h,
                                "temp",
                                "Temperature",
                                "°C",
                                tmin,
                                tmax,
                                TEMP_C,
                                &hist.temp,
                                true,
                            );
                        });
                        ui.add_space(gap);

                        // Bottom row: FSOT | science+trits | cores
                        let w1 = avail_w * 0.38;
                        let w2 = avail_w * 0.36;
                        let w3 = avail_w - w1 - w2 - gap * 2.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            fsot_cell(ui, w1, bot_h, hist);
                            science_mini_cell(ui, w2, bot_h, &self.engine);
                            cores_cell(ui, w3.max(140.0), bot_h, &self.engine);
                        });
                    }
                    FocusPanel::Cpu => {
                        graph_cell(
                            ui,
                            avail_w,
                            body_h * 0.72,
                            "cpu_full",
                            "CPU utilization",
                            "%",
                            0.0,
                            100.0,
                            CPU_C,
                            &hist.cpu,
                            true,
                        );
                        ui.add_space(6.0);
                        cores_cell(ui, avail_w, body_h * 0.26, &self.engine);
                    }
                    FocusPanel::Mem => {
                        graph_cell(
                            ui,
                            avail_w,
                            body_h,
                            "mem_full",
                            "Memory",
                            "%",
                            0.0,
                            100.0,
                            MEM_C,
                            &hist.mem,
                            true,
                        );
                    }
                    FocusPanel::Gpu => {
                        graph_cell(
                            ui,
                            avail_w,
                            body_h,
                            "gpu_full",
                            "GPU utilization",
                            "%",
                            0.0,
                            100.0,
                            GPU_C,
                            &hist.gpu,
                            true,
                        );
                    }
                    FocusPanel::Temp => {
                        graph_cell(
                            ui,
                            avail_w,
                            body_h,
                            "temp_full",
                            "Temperature",
                            "°C",
                            tmin,
                            tmax,
                            TEMP_C,
                            &hist.temp,
                            true,
                        );
                    }
                    FocusPanel::Fsot => {
                        let h1 = body_h * 0.55;
                        let h2 = body_h - h1 - 6.0;
                        fsot_cell(ui, avail_w, h1, hist);
                        ui.add_space(6.0);
                        trit_cell(ui, avail_w, h2, &self.engine);
                    }
                    FocusPanel::Science => {
                        science_full_cell(ui, avail_w, body_h, &self.engine);
                    }
                }
            });

        if want_export {
            let _ = self.engine.export_run();
        }
        if let Some(on) = want_stream {
            self.engine.stream.set_enabled(on);
        }
        if let Some(on) = want_on_top {
            self.always_on_top = on;
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
        }
        if let Some(on) = want_compact {
            self.compact = on;
            if on {
                self.focus = FocusPanel::Overview;
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(720.0, 160.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(520.0, 120.0)));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(1180.0, 780.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(900.0, 600.0)));
            }
        }
    }
}

// --------------- cells (exact-size) ---------------

/// Metric card. Returns true if clicked.
fn metric_card(
    ui: &mut Ui,
    width: f32,
    height: f32,
    title: &str,
    value: &str,
    subtitle: &str,
    frac: f64,
    accent: Color32,
    value_color: Color32,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let painter = ui.painter_at(rect);
    let hover = resp.hovered();
    let fill = if hover { PANEL_HI } else { PANEL };

    painter.rect_filled(rect, CornerRadius::same(8), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, if hover { accent } else { BORDER }),
        egui::StrokeKind::Outside,
    );

    let mut bar = rect;
    bar.set_width(4.0);
    painter.rect_filled(
        bar,
        CornerRadius {
            nw: 8,
            sw: 8,
            ne: 0,
            se: 0,
        },
        accent,
    );

    let pad = rect.shrink2(Vec2::new(12.0, 8.0));
    painter.text(
        pad.left_top() + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        title,
        FontId::proportional(11.0),
        MUTED,
    );
    painter.text(
        pad.left_top() + Vec2::new(4.0, 20.0),
        egui::Align2::LEFT_TOP,
        value,
        FontId::proportional(26.0),
        value_color,
    );
    painter.text(
        pad.left_bottom() + Vec2::new(4.0, -16.0),
        egui::Align2::LEFT_BOTTOM,
        subtitle,
        FontId::proportional(10.5),
        MUTED,
    );

    let mut track = rect.shrink2(Vec2::new(12.0, 8.0));
    track.min.y = track.max.y - 5.0;
    painter.rect_filled(track, CornerRadius::same(2), Color32::from_rgb(22, 26, 34));
    let mut fill_bar = track;
    fill_bar.set_width((track.width() * frac.clamp(0.0, 1.0) as f32).max(2.0));
    painter.rect_filled(fill_bar, CornerRadius::same(2), accent);

    resp.clone()
        .on_hover_text("Click to expand this metric");
    resp.clicked()
}

fn graph_cell(
    ui: &mut Ui,
    width: f32,
    height: f32,
    id: &str,
    title: &str,
    unit: &str,
    y_min: f64,
    y_max: f64,
    color: Color32,
    series: &VecDeque<f64>,
    fill: bool,
) {
    let size = Vec2::new(width, height);
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);

        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                // Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).strong().color(color).size(13.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let last = series.iter().rev().find(|v| v.is_finite()).copied();
                        if let Some(v) = last {
                            ui.label(
                                RichText::new(format!("{v:.1} {unit}"))
                                    .color(TEXT)
                                    .size(13.0)
                                    .strong(),
                            );
                        } else {
                            ui.label(RichText::new("n/a").color(MUTED));
                        }
                    });
                });
                ui.add_space(2.0);

                let plot_w = ui.available_width();
                let plot_h = (ui.available_height() - 2.0).max(40.0);
                filled_line_plot(ui, id, plot_w, plot_h, title, unit, y_min, y_max, color, series, fill);
            });
    });
}

fn filled_line_plot(
    ui: &mut Ui,
    id: &str,
    width: f32,
    height: f32,
    name: &str,
    unit: &str,
    y_min: f64,
    y_max: f64,
    color: Color32,
    series: &VecDeque<f64>,
    fill: bool,
) {
    let points = right_aligned_points(series);
    let mut line = Line::new(points).color(color).width(2.2).name(name);
    if fill {
        line = line.fill(y_min as f32).fill_alpha(0.22);
    }

    let y_fmt_unit = unit.to_string();
    Plot::new(Id::new(id))
        .width(width)
        .height(height)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .set_margin_fraction(Vec2::new(0.0, 0.06))
        .include_x(0.0)
        .include_x((HISTORY_LEN.saturating_sub(1)) as f64)
        .include_y(y_min)
        .include_y(y_max)
        .auto_bounds(Vec2b::FALSE)
        .show_axes([false, true])
        .show_grid(true)
        .show_background(true)
        .y_axis_min_width(28.0)
        .y_axis_formatter(move |mark, _range| {
            if y_fmt_unit == "°C" {
                format!("{:.0}", mark.value)
            } else {
                format!("{:.0}", mark.value)
            }
        })
        .y_grid_spacer(move |input| {
            let (ymin, ymax) = input.bounds;
            let step = nice_step(ymax - ymin);
            let mut marks = Vec::new();
            if step <= 0.0 {
                return marks;
            }
            let mut v = (ymin / step).floor() * step;
            while v <= ymax + step * 0.5 {
                marks.push(GridMark {
                    value: v,
                    step_size: step,
                });
                v += step;
            }
            marks
        })
        .show(ui, |plot_ui| {
            plot_ui.line(line);
        });
}

fn fsot_cell(ui: &mut Ui, width: f32, height: f32, hist: &crate::monitor::History) {
    let size = Vec2::new(width, height);
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("FSOT scalar S").strong().color(S_C).size(13.0));
                    ui.label(
                        RichText::new("  K(T1+T2+T3) · [−3, +3]")
                            .color(MUTED)
                            .size(11.0),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(v) = hist.scalar_s.back() {
                            ui.label(
                                RichText::new(format!("{v:+.3}"))
                                    .color(s_color(*v))
                                    .strong()
                                    .size(13.0),
                            );
                        }
                    });
                });
                ui.add_space(2.0);

                let plot_w = ui.available_width();
                let plot_h = (ui.available_height() - 2.0).max(40.0);
                let series = [
                    ("S", S_C, &hist.scalar_s),
                    ("T1", T1_C, &hist.t1),
                    ("T2", T2_C, &hist.t2),
                    ("T3", T3_C, &hist.t3),
                ];

                Plot::new("fsot_multi")
                    .width(plot_w)
                    .height(plot_h)
                    .allow_zoom(false)
                    .allow_drag(false)
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .set_margin_fraction(Vec2::new(0.0, 0.06))
                    .include_x(0.0)
                    .include_x((HISTORY_LEN.saturating_sub(1)) as f64)
                    .include_y(-3.0)
                    .include_y(3.0)
                    .auto_bounds(Vec2b::FALSE)
                    .show_axes([false, true])
                    .show_grid(true)
                    .legend(egui_plot::Legend::default().position(egui_plot::Corner::RightTop))
                    .y_axis_min_width(28.0)
                    .show(ui, |plot_ui| {
                        for (name, color, data) in series {
                            let pts = right_aligned_points(data);
                            plot_ui.line(Line::new(pts).color(color).width(1.8).name(name));
                        }
                        let zero = right_aligned_const(0.0);
                        plot_ui.line(
                            Line::new(zero)
                                .color(Color32::from_rgb(70, 75, 90))
                                .width(1.0)
                                .name("0"),
                        );
                    });
            });
    });
}

fn trit_cell(ui: &mut Ui, width: f32, height: f32, engine: &Engine) {
    let size = Vec2::new(width, height);
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(RichText::new("Trinary state").strong().color(TEXT).size(13.0));
                ui.label(
                    RichText::new(format!(
                        "{{-1,0,+1}} · fold {FOLD_ID} · bridge {BRIDGE_VERSION}"
                    ))
                    .color(MUTED)
                    .size(11.0),
                );
                ui.add_space(6.0);
                if let Some(s) = engine.latest.as_ref() {
                    trit_row(ui, "CPU", s.fsot.trit_cpu);
                    trit_row(ui, "MEM", s.fsot.trit_mem);
                    trit_row(ui, "GPU", s.fsot.trit_gpu);
                    trit_row(ui, "TEMP", s.fsot.trit_temp);
                    trit_row(ui, "S", s.fsot.trit_s);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    kv(ui, "load", &format!("{:.4}", s.fsot.load));
                    kv(ui, "δψ", &format!("{:+.4}", s.fsot.delta_psi));
                    kv(ui, "δθ", &format!("{:+.4}", s.fsot.delta_theta));
                    kv(ui, "hits", &format!("{:.3}", s.fsot.recent_hits));
                    kv(ui, "ρ", &format!("{:.3}", s.fsot.rho));
                    kv(ui, "S−S₀", &format!("{:+.4}", s.fsot.scalar_s - s.fsot.s_ref));
                    ui.add_space(4.0);
                    kv(ui, "ΔCPU", &format!("{:+.1} pp", s.delta.cpu_pct_points));
                    kv(ui, "ΔRAM", &format!("{:+.2} GB", s.delta.mem_gb));
                    if let Some(td) = s.delta.temp_c {
                        kv(ui, "ΔT", &format!("{:+.1} °C", td));
                    }
                }
            });
    });
}

fn certificate_strip(ui: &mut Ui, engine: &Engine) {
    Frame::new()
        .fill(PANEL_HI)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "CERTIFICATE  pin {PIN_TAG}  ·  fold {FOLD_ID}  ·  bridge {BRIDGE_VERSION}  ·  instrument {INSTRUMENT_VERSION}"
                    ))
                    .color(S_C)
                    .monospace()
                    .size(11.0)
                    .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "D_eff={:.4}  N={:.0}  P={:.4}  S_ref={:+.4}  host={}",
                            PLANT_D_EFF, PLANT_N, PLANT_P, engine.s_ref, engine.host_name
                        ))
                        .color(MUTED)
                        .monospace()
                        .size(11.0),
                    );
                });
            });
        });
    ui.add_space(4.0);
}

fn science_mini_cell(ui: &mut Ui, width: f32, height: f32, engine: &Engine) {
    let size = Vec2::new(width, height);
    let sci = engine.science();
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(RichText::new("Science (session)").strong().color(S_C).size(13.0));
                ui.label(
                    RichText::new("noise · excursions · recovery · trit dwell · corr")
                        .color(MUTED)
                        .size(11.0),
                );
                ui.add_space(4.0);
                kv(
                    ui,
                    "n",
                    &format!("{} samples · {:.0}s", sci.samples, sci.elapsed_s),
                );
                kv(
                    ui,
                    "σ_idle",
                    &sci.noise_floor_s
                        .map(|v| format!("{v:.5}"))
                        .unwrap_or_else(|| "calibrating…".into()),
                );
                kv(ui, "S_ref", &format!("{:.4}", sci.s_ref));
                kv(
                    ui,
                    "exc/h",
                    &format!("{:.2} (n={})", sci.excursion_rate_per_h, sci.excursion_count),
                );
                kv(
                    ui,
                    "τ_rec",
                    &sci.recovery_tau_s
                        .map(|v| format!("{v:.1}s"))
                        .unwrap_or_else(|| "—".into()),
                );
                kv(
                    ui,
                    "dwell",
                    &format!(
                        "−{:.0}%  0 {:.0}%  +{:.0}%",
                        100.0 * sci.trit_dwell_s_neg,
                        100.0 * sci.trit_dwell_s_zero,
                        100.0 * sci.trit_dwell_s_pos
                    ),
                );
                kv(
                    ui,
                    "r_cpu",
                    &sci.corr_cpu_s
                        .map(|v| format!("{v:+.3}"))
                        .unwrap_or_else(|| "—".into()),
                );
                kv(
                    ui,
                    "r_gpu",
                    &sci.corr_gpu_s
                        .map(|v| format!("{v:+.3}"))
                        .unwrap_or_else(|| "—".into()),
                );
                kv(
                    ui,
                    "r_temp",
                    &sci.corr_temp_s
                        .map(|v| format!("{v:+.3}"))
                        .unwrap_or_else(|| "—".into()),
                );
                ui.add_space(6.0);
                if let Some(s) = engine.latest.as_ref() {
                    trit_row(ui, "S", s.fsot.trit_s);
                    kv(ui, "δψ", &format!("{:+.4}", s.fsot.delta_psi));
                    kv(ui, "δθ", &format!("{:+.4}", s.fsot.delta_theta));
                }
            });
    });
}

fn science_full_cell(ui: &mut Ui, width: f32, height: f32, engine: &Engine) {
    let size = Vec2::new(width, height);
    let sci = engine.science();
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Scientific session metrics")
                        .strong()
                        .color(S_C)
                        .size(16.0),
                );
                ui.label(
                    RichText::new(format!(
                        "Fold {FOLD_ID} · pin {PIN_TAG} · live host sensors → preregistered bridge → S"
                    ))
                    .color(MUTED)
                    .size(12.0),
                );
                ui.add_space(10.0);

                ui.columns(3, |cols| {
                    cols[0].label(RichText::new("Identity").strong().color(TEXT));
                    kv(&mut cols[0], "pin", PIN_TAG);
                    kv(&mut cols[0], "fold", FOLD_ID);
                    kv(&mut cols[0], "bridge", BRIDGE_VERSION);
                    kv(&mut cols[0], "D_eff", &format!("{PLANT_D_EFF:.6}"));
                    kv(&mut cols[0], "N,P", &format!("{PLANT_N:.0}, {PLANT_P:.6}"));
                    kv(&mut cols[0], "host", &engine.host_name);
                    kv(&mut cols[0], "start", &engine.session_start);

                    cols[1].label(RichText::new("Dynamics").strong().color(TEXT));
                    kv(
                        &mut cols[1],
                        "samples",
                        &format!("{} ({:.0}s)", sci.samples, sci.elapsed_s),
                    );
                    kv(
                        &mut cols[1],
                        "noise σ",
                        &sci.noise_floor_s
                            .map(|v| format!("{v:.6}"))
                            .unwrap_or_else(|| "calibrating (30 idle)".into()),
                    );
                    kv(
                        &mut cols[1],
                        "idle μ",
                        &sci.idle_mean_s
                            .map(|v| format!("{v:.6}"))
                            .unwrap_or_else(|| "—".into()),
                    );
                    kv(&mut cols[1], "S_ref", &format!("{:.6}", sci.s_ref));
                    kv(
                        &mut cols[1],
                        "excursions",
                        &format!("{} ({:.2}/h)", sci.excursion_count, sci.excursion_rate_per_h),
                    );
                    kv(
                        &mut cols[1],
                        "τ_recov",
                        &sci.recovery_tau_s
                            .map(|v| format!("{v:.2} s mean"))
                            .unwrap_or_else(|| "no recovery event yet".into()),
                    );
                    kv(
                        &mut cols[1],
                        "band",
                        &format!("|S−S_ref| > {EXCURSION_BAND:.4} (seed K)"),
                    );

                    cols[2].label(RichText::new("Structure").strong().color(TEXT));
                    kv(
                        &mut cols[2],
                        "dwell−",
                        &format!("{:.1}%", 100.0 * sci.trit_dwell_s_neg),
                    );
                    kv(
                        &mut cols[2],
                        "dwell0",
                        &format!("{:.1}%", 100.0 * sci.trit_dwell_s_zero),
                    );
                    kv(
                        &mut cols[2],
                        "dwell+",
                        &format!("{:.1}%", 100.0 * sci.trit_dwell_s_pos),
                    );
                    kv(
                        &mut cols[2],
                        "r(cpu,S)",
                        &sci.corr_cpu_s
                            .map(|v| format!("{v:+.4}"))
                            .unwrap_or_else(|| "need ≥8 samples".into()),
                    );
                    kv(
                        &mut cols[2],
                        "r(gpu,S)",
                        &sci.corr_gpu_s
                            .map(|v| format!("{v:+.4}"))
                            .unwrap_or_else(|| "—".into()),
                    );
                    kv(
                        &mut cols[2],
                        "r(temp,S)",
                        &sci.corr_temp_s
                            .map(|v| format!("{v:+.4}"))
                            .unwrap_or_else(|| "—".into()),
                    );
                    kv(
                        &mut cols[2],
                        "best lag",
                        &match (sci.corr_cpu_s_best_lag, sci.corr_cpu_s_best_r) {
                            (Some(l), Some(r)) => format!("{l} s · r={r:+.3}"),
                            _ => "—".into(),
                        },
                    );
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Same FSOT spine as every other fold (pin {PIN_TAG}). \
                         This instrument is the PC host plant: sensors → bridge → S. \
                         Export a run when you want certificate.json + log SHA-256."
                    ))
                    .color(MUTED)
                    .size(12.0),
                );
            });
    });
}

fn cores_cell(ui: &mut Ui, width: f32, height: f32, engine: &Engine) {
    let size = Vec2::new(width, height);
    ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(RichText::new("CPU cores").strong().color(CPU_C).size(13.0));
                if let Some(s) = engine.latest.as_ref() {
                    ui.label(
                        RichText::new(format!("{} logical processors", s.cpu_per_core.len()))
                            .color(MUTED)
                            .size(11.0),
                    );
                    let plot_w = ui.available_width();
                    let plot_h = (ui.available_height() - 2.0).max(40.0);
                    let bars: Vec<Bar> = s
                        .cpu_per_core
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            let c = load_color(*v as f64);
                            Bar::new(i as f64, *v as f64).width(0.85).fill(c)
                        })
                        .collect();
                    Plot::new("cores")
                        .width(plot_w)
                        .height(plot_h)
                        .allow_zoom(false)
                        .allow_drag(false)
                        .allow_scroll(false)
                        .include_y(0.0)
                        .include_y(100.0)
                        .auto_bounds(Vec2b::new(true, false))
                        .show_axes([false, true])
                        .show_grid(false)
                        .set_margin_fraction(Vec2::new(0.02, 0.05))
                        .y_axis_min_width(24.0)
                        .show(ui, |plot_ui| {
                            plot_ui.bar_chart(BarChart::new(bars).name("cores"));
                        });
                }
            });
    });
}

// --------------- helpers ---------------

/// Map history into a fixed [0, HISTORY_LEN) window with newest on the right.
fn right_aligned_points(series: &VecDeque<f64>) -> PlotPoints<'static> {
    let n = series.len();
    let start = HISTORY_LEN.saturating_sub(n);
    let pts: Vec<[f64; 2]> = series
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .map(|(i, v)| [(start + i) as f64, *v])
        .collect();
    PlotPoints::from(pts)
}

fn right_aligned_const(y: f64) -> PlotPoints<'static> {
    let pts = vec![[0.0, y], [(HISTORY_LEN.saturating_sub(1)) as f64, y]];
    PlotPoints::from(pts)
}

fn trit_row(ui: &mut Ui, label: &str, t: i8) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{label:4}"))
                .color(MUTED)
                .monospace()
                .size(12.0),
        );
        let (text, color) = match t {
            -1 => ("NEG  (−1)", NEG_C),
            1 => ("POS  (+1)", POS_C),
            _ => ("ZERO ( 0)", ZERO_C),
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(112.0, 22.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(22, 26, 34));
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, color),
            egui::StrokeKind::Outside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::monospace(12.0),
            color,
        );
    });
}

fn kv(ui: &mut Ui, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{k:5}"))
                .color(MUTED)
                .monospace()
                .size(12.0),
        );
        ui.label(RichText::new(v).color(TEXT).monospace().size(12.0));
    });
}

fn alert_message(engine: &Engine, flashing: bool) -> Option<String> {
    let s = engine.latest.as_ref()?;
    let mut parts = Vec::new();
    if s.cpu_pct >= 90.0 {
        parts.push(format!("CPU high ({:.0}%)", s.cpu_pct));
    }
    if s.gpu_pct >= 95.0 {
        parts.push(format!("GPU high ({:.0}%)", s.gpu_pct));
    }
    if let Some(t) = s.temp_c {
        if t >= 85.0 {
            parts.push(format!("Temp hot ({t:.0}°C)"));
        }
    }
    if s.fsot.trit_temp > 0 {
        parts.push("Temp trit POS (ΔT vs V0)".into());
    }
    let ds = (s.fsot.scalar_s - s.fsot.s_ref).abs();
    if ds > EXCURSION_BAND {
        parts.push(format!(
            "S excursion |S−S_ref|={ds:.3} > band {EXCURSION_BAND:.3}"
        ));
    }
    if parts.is_empty() {
        return None;
    }
    let prefix = if flashing { "⚠ ALERT · " } else { "● " };
    Some(format!("{prefix}{}", parts.join(" · ")))
}

fn load_color(pct: f64) -> Color32 {
    if pct >= 85.0 {
        HOT
    } else if pct >= 60.0 {
        WARN
    } else {
        GOOD
    }
}

fn temp_color(c: f64) -> Color32 {
    if c >= 85.0 {
        HOT
    } else if c >= 70.0 {
        WARN
    } else {
        GOOD
    }
}

fn s_color(s: f64) -> Color32 {
    if s.abs() > 1.5 {
        WARN
    } else {
        S_C
    }
}

fn temp_axis_range(series: &VecDeque<f64>) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for v in series {
        if v.is_finite() {
            min = min.min(*v);
            max = max.max(*v);
        }
    }
    if !min.is_finite() {
        return (20.0, 90.0);
    }
    // Keep a usable window even when temp is flat
    let span = (max - min).max(10.0);
    let mid = (min + max) / 2.0;
    let lo = (mid - span / 2.0 - 2.0).floor();
    let hi = (mid + span / 2.0 + 2.0).ceil();
    (lo, hi)
}

fn short_name(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn nice_step(range: f64) -> f64 {
    if range <= 0.0 || !range.is_finite() {
        return 1.0;
    }
    let exp = range.log10().floor();
    let base = 10f64.powf(exp);
    let frac = range / base;
    let nice = if frac < 1.5 {
        1.0
    } else if frac < 3.5 {
        2.0
    } else if frac < 7.5 {
        5.0
    } else {
        10.0
    };
    (nice * base / 2.0).max(1.0)
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.widgets.noninteractive.bg_fill = PANEL;
    style.visuals.widgets.inactive.bg_fill = PANEL_HI;
    style.visuals.override_text_color = Some(TEXT);
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    // Darker plot-friendly widgets
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    ctx.set_style(style);
}
