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

use serde::{Deserialize, Serialize};

use crate::ExportError;
use crate::report::{Chart, Report};

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

/// `--gp-bg:#fcfdfd;--gp-text:…;--gp-chart:#1a9384`: a palette as CSS custom properties.
pub fn theme_vars(_palette: &Palette) -> String {
    todo!()
}

/// The chart as one `<svg>` element (a whole file when `alone`); `""` for a chart without items.
/// Its `<desc>` lists every `label: display`, its `aria-label` is the title.
pub fn svg(_chart: &Chart, _alone: bool) -> String {
    todo!()
}

/// The chart a single-chart format draws: the one with this id, else (no id, or an unknown one)
/// the first chart with data. `ExportError::NoChart` when that chart has no items or there is none.
pub fn pick<'a>(_report: &'a Report, _id: Option<&str>) -> Result<&'a Chart, ExportError> {
    todo!()
}

/// The `svg` format: an XML declaration, then [`svg`] standalone, then a newline.
pub fn svg_file(_report: &Report, _id: Option<&str>) -> Result<String, ExportError> {
    todo!()
}

/// The `png` format: the standalone SVG rasterized at `scale` × its size (the dashboard used 2×),
/// on the light palette. `ExportError::NoChart` for a chart without items; `ExportError::Raster` if
/// drawing fails. Needs a rasterizer dependency (resvg + tiny-skia, see the crate report).
pub fn png(_chart: &Chart, _scale: u32) -> Result<Vec<u8>, ExportError> {
    todo!()
}

/// A chart an export can be downloaded as (`GET /api/export/{kind}/charts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartInfo {
    pub id: String,
    pub title: String,
}

/// The charts of a report that have data, in report order.
pub fn chart_list(_report: &Report) -> Vec<ChartInfo> {
    todo!()
}
