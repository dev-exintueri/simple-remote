// Spike only (throwaway).
//! egui_probe: eframe 0.36.2 spike for a remote-desktop viewer.
//!
//! - Synthetic 1920x1080 RGBA frame, uploaded to one TextureHandle per new frame.
//! - Upload mode toggle: `set` (full delta, wgpu recreates the texture) vs
//!   `set_partial([0,0])` (reuses the texture, only `queue.write_texture`).
//! - Timing overlay: gen / convert / upload / whole ui() / eframe cpu_usage.
//! - Korean IME test fields and logs of Ime/Text and Key events.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{Color32, ColorImage, Event, ImeEvent, TextureHandle, TextureOptions};

const W: usize = 1920;
const H: usize = 1080;
const SAMPLES: usize = 240;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("egui_probe")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_probe",
        options,
        Box::new(|cc| Ok(Box::new(Probe::new(&cc.egui_ctx)))),
    )
}

/// Rolling window of millisecond samples.
#[derive(Default)]
struct Stat(VecDeque<f32>);

impl Stat {
    fn push(&mut self, ms: f32) {
        if self.0.len() == SAMPLES {
            self.0.pop_front();
        }
        self.0.push_back(ms);
    }

    fn summary(&self) -> String {
        if self.0.is_empty() {
            return "-".to_owned();
        }
        let mut v: Vec<f32> = self.0.iter().copied().collect();
        v.sort_by(f32::total_cmp);
        let avg = v.iter().sum::<f32>() / v.len() as f32;
        let p95 = v[((v.len() - 1) as f32 * 0.95).round() as usize];
        let max = v[v.len() - 1];
        format!("avg {avg:6.2}  p95 {p95:6.2}  max {max:6.2} ms")
    }
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

fn push_log(log: &mut VecDeque<String>, cap: usize, line: String) {
    if log.len() == cap {
        log.pop_front();
    }
    log.push_back(line);
}

struct Probe {
    tex: TextureHandle,
    rgba: Vec<u8>,
    frame_no: u64,
    pace_30fps: bool,
    use_set_partial: bool,
    next_frame_at: Instant,
    paint_times: VecDeque<f64>,
    gen_ms: Stat,
    convert_ms: Stat,
    upload_ms: Stat,
    ui_ms: Stat,
    cpu_usage_ms: Stat,
    single: String,
    multi: String,
    ime_log: VecDeque<String>,
    key_log: VecDeque<String>,
    font_note: String,
}

impl Probe {
    fn new(ctx: &egui::Context) -> Self {
        let font_note = install_korean_font(ctx);
        let rgba = vec![0u8; W * H * 4];
        let tex = ctx.load_texture(
            "video",
            ColorImage::from_rgba_unmultiplied([W, H], &rgba),
            TextureOptions::LINEAR,
        );
        Self {
            tex,
            rgba,
            frame_no: 0,
            pace_30fps: true,
            use_set_partial: true,
            next_frame_at: Instant::now(),
            paint_times: VecDeque::new(),
            gen_ms: Stat::default(),
            convert_ms: Stat::default(),
            upload_ms: Stat::default(),
            ui_ms: Stat::default(),
            cpu_usage_ms: Stat::default(),
            single: String::new(),
            multi: String::new(),
            ime_log: VecDeque::new(),
            key_log: VecDeque::new(),
            font_note,
        }
    }

    /// Moving gradient plus a white frame-counter bar across the top 40 rows.
    fn generate(&mut self) {
        let n = self.frame_no as usize;
        let bar = n % W;
        let blue = (n * 3 % 256) as u8;
        for (y, row) in self.rgba.chunks_exact_mut(W * 4).enumerate() {
            let g = ((y + n) & 0xff) as u8;
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                let white = y < 40 && x <= bar;
                if white {
                    px.copy_from_slice(&[255, 255, 255, 255]);
                } else {
                    px.copy_from_slice(&[((x + 2 * n) & 0xff) as u8, g, blue, 255]);
                }
            }
        }
    }

    fn produce_frame(&mut self) {
        let t0 = Instant::now();
        self.generate();
        let t1 = Instant::now();
        let image = ColorImage::from_rgba_unmultiplied([W, H], &self.rgba);
        let t2 = Instant::now();
        if self.use_set_partial {
            self.tex.set_partial([0, 0], image, TextureOptions::LINEAR);
        } else {
            self.tex.set(image, TextureOptions::LINEAR);
        }
        let t3 = Instant::now();
        self.gen_ms.push(ms(t1 - t0));
        self.convert_ms.push(ms(t2 - t1));
        self.upload_ms.push(ms(t3 - t2));
        self.frame_no += 1;
    }

    fn collect_events(&mut self, ctx: &egui::Context) {
        let (time, events) = ctx.input(|i| (i.time, i.events.clone()));
        for ev in events {
            match ev {
                Event::Ime(ime) => {
                    let line = match ime {
                        ImeEvent::Preedit {
                            text,
                            active_range_chars,
                        } => format!("Preedit({text:?}, {active_range_chars:?})"),
                        ImeEvent::Commit(text) => format!("Commit({text:?})"),
                        ImeEvent::DeleteSurrounding {
                            before_chars,
                            after_chars,
                        } => format!("DeleteSurrounding({before_chars}, {after_chars})"),
                        #[expect(deprecated)]
                        ImeEvent::Enabled => "Enabled".to_owned(),
                        #[expect(deprecated)]
                        ImeEvent::Disabled => "Disabled".to_owned(),
                    };
                    push_log(&mut self.ime_log, 20, format!("{time:9.3} Ime {line}"));
                }
                Event::Text(text) => {
                    push_log(&mut self.ime_log, 20, format!("{time:9.3} Text({text:?})"));
                }
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    repeat,
                    modifiers,
                } => {
                    push_log(
                        &mut self.key_log,
                        10,
                        format!(
                            "{time:9.3} {key:?} phys={physical_key:?} pressed={pressed} repeat={repeat} mods={modifiers:?}"
                        ),
                    );
                }
                _ => {}
            }
        }
    }
}

/// egui's default fonts have no Hangul glyphs, so load a system font.
fn install_korean_font(ctx: &egui::Context) -> String {
    let candidates = [
        r"C:\Windows\Fonts\malgun.ttf",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            ctx.add_font(FontInsert::new(
                "korean",
                egui::FontData::from_owned(bytes),
                vec![
                    InsertFontFamily {
                        family: egui::FontFamily::Proportional,
                        priority: FontPriority::Lowest,
                    },
                    InsertFontFamily {
                        family: egui::FontFamily::Monospace,
                        priority: FontPriority::Lowest,
                    },
                ],
            ));
            return format!("Korean font: {path}");
        }
    }
    "Korean font: NOT FOUND (Hangul will render as boxes)".to_owned()
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ui_start = Instant::now();
        let ctx = ui.ctx().clone();

        if let Some(cpu) = frame.info().cpu_usage {
            self.cpu_usage_ms.push(cpu * 1000.0);
        }

        let now_t = ctx.input(|i| i.time);
        self.paint_times.push_back(now_t);
        while self.paint_times.front().is_some_and(|t| now_t - t > 1.0) {
            self.paint_times.pop_front();
        }

        self.collect_events(&ctx);

        let now = Instant::now();
        if !self.pace_30fps || now >= self.next_frame_at {
            self.produce_frame();
            self.next_frame_at = if now > self.next_frame_at + Duration::from_millis(100) {
                now + Duration::from_secs_f64(1.0 / 30.0)
            } else {
                self.next_frame_at + Duration::from_secs_f64(1.0 / 30.0)
            };
        }
        if self.pace_30fps {
            ctx.request_repaint_after(self.next_frame_at.saturating_duration_since(Instant::now()));
        } else {
            ctx.request_repaint();
        }

        egui::Panel::right("side")
            .resizable(true)
            .default_size(460.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (ppp, native_ppp, dt) = ctx.input(|i| {
                        (i.pixels_per_point, i.viewport().native_pixels_per_point, i.unstable_dt)
                    });
                    ui.monospace(format!("paints/s  {}", self.paint_times.len()));
                    ui.monospace(format!("video frames  {}", self.frame_no));
                    ui.monospace(format!("unstable_dt  {:.2} ms", dt * 1000.0));
                    ui.monospace(format!(
                        "pixels_per_point {ppp:.3}  native {native_ppp:?}"
                    ));
                    ui.monospace(format!("gen      {}", self.gen_ms.summary()));
                    ui.monospace(format!("convert  {}", self.convert_ms.summary()));
                    ui.monospace(format!("upload   {}", self.upload_ms.summary()));
                    ui.monospace(format!("ui()     {}", self.ui_ms.summary()));
                    ui.monospace(format!("cpu_usage{}", self.cpu_usage_ms.summary()));
                    ui.label("(cpu_usage = eframe's previous-frame ui()+render, vsync excluded on wgpu)");
                    ui.checkbox(&mut self.pace_30fps, "pace video at 30 fps (else every repaint)");
                    ui.checkbox(
                        &mut self.use_set_partial,
                        "upload with set_partial([0,0]) (else set = new GPU texture)",
                    );
                    ui.label(&self.font_note);
                    ui.separator();

                    ui.label("single-line:");
                    ui.add(egui::TextEdit::singleline(&mut self.single).desired_width(f32::INFINITY));
                    ui.label("multi-line:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.multi)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    ui.separator();

                    ui.label("last 20 Ime/Text events:");
                    for line in &self.ime_log {
                        ui.monospace(line);
                    }
                    ui.separator();
                    ui.label("last 10 Key events:");
                    for line in &self.key_log {
                        ui.monospace(line);
                    }
                });
            });

        egui::CentralPanel::no_frame().show(ui, |ui| {
            let avail = ui.available_rect_before_wrap();
            let scale = (avail.width() / W as f32).min(avail.height() / H as f32);
            let size = egui::vec2(W as f32 * scale, H as f32 * scale);
            let rect = egui::Rect::from_center_size(avail.center(), size);
            ui.painter().rect_filled(avail, 0.0, Color32::BLACK);
            egui::Image::from_texture(&self.tex).paint_at(ui, rect);
        });

        self.ui_ms.push(ms(ui_start.elapsed()));
    }
}
