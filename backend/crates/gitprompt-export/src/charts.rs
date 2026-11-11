//! Charts as SVG (ADR-0018): drawn from a report [`Chart`] with no script and no external file.
//! Colours are CSS variables, so an inline chart follows the light or dark theme of the page around
//! it; a standalone chart (the `svg` format, and the `png` drawn from it) carries both themes.
//!
//! ```text
//! svg(chart, false)   for a page that defines the variables (html::style)
//! svg(chart, true)    a file of its own: namespace, size, title, background and both themes
//! ```
//!
//! Geometry (port of `export/charts.py`): bars are 360 wide, 34 per row (label, value, an 8-high
//! track and bar of width `max(2, 360·value/peak)`); columns fit 480×180 (plot 140 high), each
//! 72 % of its step, with the peak's value on top and at most three axis labels (first, middle,
//! last) once there are more than four items. Coordinates are written with one decimal.

use std::fmt::Write;
use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};

use crate::ExportError;
use crate::escape::html as esc;
use crate::report::{Bar, Chart, ChartKind, Report};

pub const BAR_W: u32 = 360;
pub const ROW_H: u32 = 34;
pub const COL_W: u32 = 480;
pub const COL_H: u32 = 180;
/// Characters of a bar label drawn before it is clipped with `…` (the `<title>` keeps all of it).
pub const LABEL_CHARS: usize = 36;
/// The PNG is drawn at this multiple of the SVG's size.
pub const PNG_SCALE: u32 = 2;
pub const FONT: &str = "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif";

/// One palette of the dashboard's chart slots and neutrals (web/src/lib/design/tokens.css).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub grid: &'static str,
    pub axis: &'static str,
    pub chart: &'static str,
}

pub const LIGHT: Palette = Palette { bg: "#fcfdfd", text: "#151c1b", muted: "#5f6c69", grid: "#e6ebea", axis: "#c9d2d0", chart: "#1a9384" };
pub const DARK: Palette = Palette { bg: "#151c1b", text: "#e6ecea", muted: "#8e9b98", grid: "#212a28", axis: "#3b4745", chart: "#2fa898" };

/// The classes a chart uses (`gp-bar`, `gp-track`, `gp-axis`, `gp-text`, `gp-muted`, `gp-num`).
pub const CHART_STYLE: &str = ".gp-bar{fill:var(--gp-chart)}.gp-track{fill:var(--gp-grid)}.gp-axis{stroke:var(--gp-axis);stroke-width:1}.gp-text{fill:var(--gp-text)}.gp-muted{fill:var(--gp-muted)}.gp-num{font-variant-numeric:tabular-nums}";

impl Palette {
    /// The palette's slots by name, in CSS order.
    fn entries(&self) -> [(&'static str, &'static str); 6] {
        [("bg", self.bg), ("text", self.text), ("muted", self.muted), ("grid", self.grid), ("axis", self.axis), ("chart", self.chart)]
    }
}

/// `--gp-bg:#fcfdfd;--gp-text:…;--gp-chart:#1a9384`: a palette as CSS custom properties.
pub fn theme_vars(palette: &Palette) -> String {
    palette.entries().iter().map(|(name, value)| format!("--gp-{name}:{value}")).collect::<Vec<_>>().join(";")
}

/// How a standalone chart is coloured.
#[derive(Clone, Copy)]
enum Paint {
    /// CSS variables, both themes (the `svg` format: follows the viewer's setting).
    Themes,
    /// One palette written out (the PNG: the rasterizer has no CSS variables or media queries).
    Fixed(&'static Palette),
}

impl Paint {
    fn style(self) -> String {
        match self {
            Paint::Themes => {
                format!(":root{{{}}}@media (prefers-color-scheme: dark){{:root{{{}}}}}{CHART_STYLE}", theme_vars(&LIGHT), theme_vars(&DARK))
            }
            Paint::Fixed(palette) => resolve(CHART_STYLE, palette),
        }
    }

    fn background(self) -> String {
        match self {
            Paint::Themes => "var(--gp-bg)".to_string(),
            Paint::Fixed(palette) => palette.bg.to_string(),
        }
    }
}

/// `css` with every `var(--gp-<slot>)` replaced by the palette's colour.
fn resolve(css: &str, palette: &Palette) -> String {
    palette.entries().iter().fold(css.to_string(), |css, (name, value)| css.replace(&format!("var(--gp-{name})"), value))
}

/// The chart as one `<svg>` element (a whole file when `alone`); `""` for a chart without items.
/// Its `<desc>` lists every `label: display`, its `aria-label` is the title.
pub fn svg(chart: &Chart, alone: bool) -> String {
    draw(chart, alone.then_some(Paint::Themes))
}

/// The geometry of a drawn chart: its marks and the size they take.
struct Marks {
    svg: String,
    width: i64,
    height: i64,
}

/// [`svg`], standalone when `paint` is given.
fn draw(chart: &Chart, paint: Option<Paint>) -> String {
    if chart.items.is_empty() {
        return String::new();
    }
    let top = if paint.is_some() { 36 } else { 0 };
    let Marks { svg: marks, width, height } = match chart.kind {
        ChartKind::Bars => bars(&chart.items, top),
        ChartKind::Columns => columns(&chart.items, top),
    };
    let pad: i64 = if paint.is_some() { 16 } else { 0 };
    let (w, h) = (width + 2 * pad, height + 2 * pad);
    let desc = chart.items.iter().map(|b| format!("{}: {}", b.label, b.display)).collect::<Vec<_>>().join("; ");
    let title = esc(&chart.title);
    // A file of its own needs the namespace and a size; inline in HTML, neither (no URL in the page).
    let (size, head) = match paint {
        Some(paint) => (
            format!(r#" xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}""#),
            format!(
                r#"<style>{}</style><rect x="{}" y="{}" width="{w}" height="{h}" fill="{}"/><text class="gp-text" x="0" y="14" font-size="16" font-weight="600">{title}</text>"#,
                paint.style(),
                -pad,
                -pad,
                paint.background(),
            ),
        ),
        None => (String::new(), String::new()),
    };
    format!(
        r#"<svg class="gp-{}" viewBox="{} {} {w} {h}"{size} role="img" font-family="{FONT}" aria-label="{title}"><desc>{}</desc>{head}{marks}</svg>"#,
        chart.kind.as_str(),
        -pad,
        -pad,
        esc(&desc),
    )
}

/// The scale's top: the largest value, or 1 when none is above zero.
fn peak(items: &[Bar]) -> f64 {
    let max = items.iter().map(|b| b.value).fold(f64::NEG_INFINITY, f64::max);
    if max == 0.0 || !max.is_finite() { 1.0 } else { max }
}

/// `text` cut to [`LABEL_CHARS`] characters, ending with `…` when cut.
fn clip(text: &str) -> String {
    if text.chars().count() <= LABEL_CHARS {
        return text.to_string();
    }
    let kept: String = text.chars().take(LABEL_CHARS - 1).collect();
    format!("{}…", kept.trim_end())
}

// `write!` into a `String` cannot fail (`fmt::Write for String` always returns `Ok`), hence the
// `let _ =` below.

fn bars(items: &[Bar], top: i64) -> Marks {
    let peak = peak(items);
    let mut marks = String::new();
    for (i, b) in (0_i64..).zip(items) {
        let y = top + i * i64::from(ROW_H);
        let w = (f64::from(BAR_W) * b.value / peak).max(2.0);
        let (label, display) = (esc(&b.label), esc(&b.display));
        let _ = write!(
            marks,
            concat!(
                r#"<g><title>{label}: {display}</title>"#,
                r#"<text class="gp-text" x="0" y="{ty}" font-size="13">{clipped}</text>"#,
                r#"<text class="gp-muted gp-num" x="{bw}" y="{ty}" font-size="13" text-anchor="end">{display}</text>"#,
                r#"<rect class="gp-track" x="0" y="{ry}" width="{bw}" height="8" rx="4"/>"#,
                r#"<rect class="gp-bar" x="0" y="{ry}" width="{w:.1}" height="8" rx="4"/></g>"#,
            ),
            label = label,
            display = display,
            clipped = esc(&clip(&b.label)),
            ty = y + 13,
            ry = y + 19,
            bw = BAR_W,
            w = w,
        );
    }
    let rows = i64::try_from(items.len()).unwrap_or(i64::MAX);
    Marks { svg: marks, width: i64::from(BAR_W), height: top + rows * i64::from(ROW_H) }
}

fn columns(items: &[Bar], top: i64) -> Marks {
    let peak = peak(items);
    let plot_h = f64::from(COL_H - 40);
    let base = top + 20 + i64::from(COL_H - 40);
    let col_w = f64::from(COL_W);
    let n = items.len();
    let step = col_w / n.max(1) as f64;
    let width = (step * 0.72).max(1.0);
    // `peak` is one of these values (their maximum), so exact equality finds it.
    #[expect(clippy::float_cmp, reason = "looking up the very value peak() returned")]
    let peak_label = items.iter().find(|b| b.value == peak).map_or_else(|| number(peak), |b| b.display.clone());
    let mut marks = format!(
        r#"<text class="gp-muted gp-num" x="0" y="{}" font-size="15">{}</text><line class="gp-axis" x1="0" x2="{COL_W}" y1="{y}" y2="{y}"/>"#,
        top + 12,
        esc(&peak_label),
        y = number(base as f64 + 0.5),
    );
    for (i, b) in items.iter().enumerate() {
        let h = plot_h * b.value / peak;
        let x = i as f64 * step + (step - width) / 2.0;
        let _ = write!(
            marks,
            r#"<rect class="gp-bar" x="{x:.1}" y="{:.1}" width="{width:.1}" height="{:.1}" rx="2"><title>{}: {}</title></rect>"#,
            base as f64 - h,
            h.max(0.0),
            esc(&b.label),
            esc(&b.display),
        );
    }
    for (i, item) in ticks(n).into_iter().filter_map(|i| Some((i, items.get(i)?))) {
        let anchor = if n > 1 && i == 0 {
            "start"
        } else if n > 1 && i == n - 1 {
            "end"
        } else {
            "middle"
        };
        let x = match anchor {
            "start" => 0.0,
            "end" => col_w,
            _ => i as f64 * step + step / 2.0,
        };
        let _ = write!(
            marks,
            r#"<text class="gp-muted" x="{x:.1}" y="{}" font-size="15" text-anchor="{anchor}">{}</text>"#,
            base + 22,
            esc(&item.label),
        );
    }
    Marks { svg: marks, width: i64::from(COL_W), height: base + 30 }
}

/// The columns labelled under the axis: all up to four, else the first, middle and last.
fn ticks(n: usize) -> Vec<usize> {
    if n > 4 {
        let mut ticks = vec![0, n / 2, n - 1];
        ticks.dedup();
        ticks
    } else {
        (0..n).collect()
    }
}

/// A number as Python's `str` writes it: whole numbers without a fraction (`1`), others as is.
#[expect(clippy::cast_possible_truncation, reason = "only whole values below 1e15, which convert exactly")]
fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 { format!("{}", value as i64) } else { format!("{value}") }
}

/// The chart a single-chart format draws: the one with this id, else (no id, or an unknown one)
/// the first chart with data. `ExportError::NoChart` when that chart has no items or there is none.
pub fn pick<'a>(report: &'a Report, id: Option<&str>) -> Result<&'a Chart, ExportError> {
    let charts = report.charts();
    let chosen = id.and_then(|id| report.chart(id)).or_else(|| charts.into_iter().find(|c| !c.items.is_empty()));
    chosen.filter(|c| !c.items.is_empty()).ok_or(ExportError::NoChart)
}

/// The `svg` format: an XML declaration, then [`svg`] standalone, then a newline.
pub fn svg_file(report: &Report, id: Option<&str>) -> Result<String, ExportError> {
    let chart = pick(report, id)?;
    Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n", svg(chart, true)))
}

/// The `png` format: the standalone SVG rasterized at `scale` × its size (the dashboard used 2×),
/// on the light palette, with resvg. Text is set in this computer's fonts (loaded once).
/// `ExportError::NoChart` for a chart without items; `ExportError::Raster` if drawing fails.
pub fn png(chart: &Chart, scale: u32) -> Result<Vec<u8>, ExportError> {
    if chart.items.is_empty() {
        return Err(ExportError::NoChart);
    }
    let source = draw(chart, Some(Paint::Fixed(&LIGHT)));
    let options = usvg::Options { fontdb: fonts(), ..usvg::Options::default() };
    let tree = usvg::Tree::from_str(&source, &options).map_err(|e| ExportError::Raster(e.to_string()))?;
    let size = tree.size().to_int_size().scale_by(scale as f32).ok_or_else(|| ExportError::Raster(format!("cannot scale by {scale}")))?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| ExportError::Raster(format!("no {}×{} canvas", size.width(), size.height())))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale as f32, scale as f32), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| ExportError::Raster(e.to_string()))
}

/// This computer's fonts, loaded on the first PNG and shared by every later one.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// A chart an export can be downloaded as (`GET /api/export/{kind}/charts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartInfo {
    pub id: String,
    pub title: String,
}

/// The charts of a report that have data, in report order.
pub fn chart_list(report: &Report) -> Vec<ChartInfo> {
    report.charts().into_iter().filter(|c| !c.items.is_empty()).map(|c| ChartInfo { id: c.id.clone(), title: c.title.clone() }).collect()
}
