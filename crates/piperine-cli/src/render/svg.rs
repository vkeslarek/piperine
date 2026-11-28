//! `SvgPlot`: a plot as one SVG document (CLI-22). The text of the SVG and nothing else: the
//! geometry (what is drawn where) is `piperine_api::Plot`'s. Per panel a `<rect>`; per tick a
//! `<line>` and a `<text>`; per axis a title `<text>`; one `<path class="series"
//! data-signal="...">` per series with one `M ... L ...` subpath per run of points; one
//! `<path class="lane" data-signal="...">` per lane; a legend. Coordinates are written with two
//! decimals and the colors come from a fixed palette by signal index, so the same plot is the
//! same bytes.

use std::io::{self, Write};

use piperine_api::plot::{Axis, Panel, Rect};
use piperine_api::{Cell, Plot};

/// The stroke of the signal at index `i`, repeating after eight.
const PALETTE: [&str; 8] = ["#1f77b4", "#d62728", "#2ca02c", "#ff7f0e", "#9467bd", "#8c564b", "#e377c2", "#17becf"];

/// The length of a tick mark.
const TICK: f64 = 6.0;

/// A plot laid out as a document a browser opens.
pub struct SvgPlot;

impl SvgPlot {
    /// The whole document.
    pub fn write(plot: &Plot, out: &mut dyn Write) -> io::Result<()> {
        writeln!(out, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"sans-serif\" font-size=\"11\">", w = plot.width, h = plot.height)?;
        for (i, panel) in plot.panels.iter().enumerate() {
            // the x axis is labelled under the lowest lane, which only a one-panel plot has
            let base = plot.lanes.last().filter(|_| i == 0).map_or(panel.area.y + panel.area.h, |lane| lane.area.y + lane.area.h);
            SvgPlot::panel(panel, base, out)?;
            for (s, series) in panel.series.iter().enumerate() {
                let mut d = String::new();
                for run in &series.runs {
                    for (k, (x, y)) in run.iter().enumerate() {
                        d.push_str(&format!("{}{x:.2} {y:.2} ", if k == 0 { "M" } else { "L" }));
                    }
                }
                writeln!(out, "<path class=\"series\" data-signal=\"{}\" d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"/>", SvgPlot::escape(&series.label), d.trim_end(), PALETTE[s % PALETTE.len()])?;
            }
        }
        let first = plot.panels.first();
        let series = first.map_or(0, |p| p.series.len());
        for (i, lane) in plot.lanes.iter().enumerate() {
            let Some(panel) = first else { break };
            writeln!(out, "<path class=\"lane\" data-signal=\"{}\" d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"/>", SvgPlot::escape(&lane.label), SvgPlot::steps(lane.area, &panel.x, &lane.steps, &panel.area), PALETTE[(series + i) % PALETTE.len()])?;
            writeln!(out, "<text class=\"lane-label\" x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"end\">{}</text>", lane.area.x - 8.0, lane.area.y + lane.area.h / 2.0 + 4.0, SvgPlot::escape(&lane.label))?;
        }
        if let Some(panel) = first {
            let labels: Vec<&str> = panel.series.iter().map(|s| s.label.as_str()).chain(plot.lanes.iter().map(|l| l.label.as_str())).collect();
            for (i, label) in labels.iter().enumerate() {
                let (x, y) = (panel.area.x + panel.area.w - 160.0, panel.area.y + 14.0 + 16.0 * i as f64);
                writeln!(out, "<line class=\"legend\" x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"2\"/>", y - 4.0, x + 18.0, y - 4.0, PALETTE[i % PALETTE.len()])?;
                writeln!(out, "<text class=\"legend-label\" x=\"{:.2}\" y=\"{y:.2}\">{}</text>", x + 24.0, SvgPlot::escape(label))?;
            }
        }
        writeln!(out, "</svg>")
    }

    /// The frame of a panel, its axes with ticks, labels and titles.
    fn panel(panel: &Panel, base: f64, out: &mut dyn Write) -> io::Result<()> {
        let a = panel.area;
        writeln!(out, "<rect class=\"panel\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"none\" stroke=\"#888888\"/>", a.x, a.y, a.w, a.h)?;
        for tick in &panel.x.ticks {
            let x = panel.x.place(tick.value, a.x, a.x + a.w);
            writeln!(out, "<line class=\"tick x\" x1=\"{x:.2}\" y1=\"{base:.2}\" x2=\"{x:.2}\" y2=\"{:.2}\" stroke=\"#444444\"/>", base + TICK)?;
            writeln!(out, "<text class=\"tick-label x\" x=\"{x:.2}\" y=\"{:.2}\" text-anchor=\"middle\">{}</text>", base + TICK + 12.0, SvgPlot::escape(&tick.label))?;
        }
        for tick in &panel.y.ticks {
            let y = panel.y.place(tick.value, a.y + a.h, a.y);
            writeln!(out, "<line class=\"tick y\" x1=\"{:.2}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" stroke=\"#444444\"/>", a.x - TICK, a.x)?;
            writeln!(out, "<text class=\"tick-label y\" x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"end\">{}</text>", a.x - TICK - 3.0, y + 4.0, SvgPlot::escape(&tick.label))?;
        }
        writeln!(out, "<text class=\"axis-title x\" x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"middle\">{}</text>", a.x + a.w / 2.0, base + TICK + 32.0, SvgPlot::escape(&panel.x.title))?;
        writeln!(out, "<text class=\"axis-title y\" x=\"{:.2}\" y=\"{:.2}\" text-anchor=\"middle\" transform=\"rotate(-90 {:.2} {:.2})\">{}</text>", 16.0, a.y + a.h / 2.0, 16.0, a.y + a.h / 2.0, SvgPlot::escape(&panel.y.title))
    }

    /// A step path: the value holds until the next abscissa. A bool is low or high; an integer
    /// is placed between the lane's smallest and largest; an unknown breaks the path.
    fn steps(lane: Rect, x: &Axis, steps: &[(f64, Cell)], area: &Rect) -> String {
        let ints: Vec<i128> = steps.iter().filter_map(|(_, c)| if let Cell::Int(v) = c { Some(*v) } else { None }).collect();
        let (lo, hi) = (ints.iter().min().copied().unwrap_or(0), ints.iter().max().copied().unwrap_or(1));
        let level = |cell: &Cell| -> Option<f64> {
            let unit = match cell {
                Cell::Bool(b) => f64::from(u8::from(*b)),
                Cell::Int(v) if hi > lo => (*v - lo) as f64 / (hi - lo) as f64,
                Cell::Int(_) => 0.5,
                _ => return None,
            };
            Some(lane.y + lane.h - 4.0 - (lane.h - 8.0) * unit)
        };
        let mut d = String::new();
        let mut previous: Option<f64> = None;
        for (at, cell) in steps {
            let px = x.place(*at, area.x, area.x + area.w);
            match (level(cell), previous) {
                (Some(y), Some(before)) => d.push_str(&format!("L{px:.2} {before:.2} L{px:.2} {y:.2} ")),
                (Some(y), None) => d.push_str(&format!("M{px:.2} {y:.2} ")),
                (None, _) => {}
            }
            previous = level(cell);
        }
        d.trim_end().to_string()
    }

    /// Text safe inside an element or an attribute.
    fn escape(text: &str) -> String {
        text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_api::{Analysis, Grid, Table};

    /// Associated test helpers of `svg`, grouped on one fixture type.
    struct SvgFixture;

    impl SvgFixture {
        fn svg(table: &Table, analysis: &Analysis) -> String {
            let plot = Plot::of(table, analysis).expect("a plot");
            let mut out = Vec::new();
            SvgPlot::write(&plot, &mut out).expect("a vector takes writes");
            String::from_utf8(out).expect("text")
        }

        fn transient() -> Analysis {
            Analysis::Transient { stop: 4.0, max_step: None, print_step: None, from: None }
        }

        /// time 0..4 and two real signals `a` (0..8) and `b` (8..0).
        fn two_signals() -> Table {
            let rows = (0..5).map(|k| vec![Cell::Real(f64::from(k)), Cell::Real(f64::from(2 * k)), Cell::Real(f64::from(8 - 2 * k))]).collect();
            Table { columns: vec!["time".into(), "Top.a".into(), "Top.b".into()], rows }
        }
    }

    #[test]
    fn one_path_per_signal_with_its_name_and_a_legend() {
        // CLI-22 AC 2: a `series` path per real signal carrying its full path in `data-signal`,
        // and a legend naming each; the first point of `Top.a` (time 0, value 0) is the plot
        // area's bottom left, (70, 430), the last (time 4, value 8) its top right, (780, 20)
        let text = SvgFixture::svg(&SvgFixture::two_signals(), &SvgFixture::transient());
        assert_eq!(text.matches("<path class=\"series\"").count(), 2, "{text}");
        let a = text.lines().find(|l| l.contains("data-signal=\"Top.a\"")).expect("a path for Top.a");
        assert!(a.contains("d=\"M70.00 430.00 L247.50 327.50 L425.00 225.00 L602.50 122.50 L780.00 20.00\""), "{a}");
        let b = text.lines().find(|l| l.contains("data-signal=\"Top.b\"")).expect("a path for Top.b");
        assert!(b.contains("d=\"M70.00 20.00 "), "{b}");
        assert_eq!(text.matches("class=\"legend-label\"").count(), 2);
        assert!(text.contains(">Top.a</text>") && text.contains(">Top.b</text>"));
        assert!(text.starts_with("<svg ") && text.trim_end().ends_with("</svg>"));
        assert_eq!(text.matches("<rect class=\"panel\"").count(), 1);
    }

    #[test]
    fn each_axis_has_ticks_labels_and_a_title() {
        // CLI-22 AC 4: x is `time` with ticks 0..4 (step 0.5 would be 9, step 1 is 5; the
        // smallest step with at most 10 ticks is 0.5), y has ticks 0..8 (step 1, 9 ticks); each
        // tick is a `<line>` and a `<text>`, and each axis has one title
        let text = SvgFixture::svg(&SvgFixture::two_signals(), &SvgFixture::transient());
        let x_ticks = text.matches("class=\"tick x\"").count();
        let y_ticks = text.matches("class=\"tick y\"").count();
        assert_eq!((x_ticks, y_ticks), (9, 9), "{text}");
        assert_eq!(text.matches("class=\"tick-label x\"").count(), 9);
        assert_eq!(text.matches("class=\"tick-label y\"").count(), 9);
        assert_eq!(text.matches("class=\"axis-title x\"").count(), 1);
        assert_eq!(text.matches("class=\"axis-title y\"").count(), 1);
        assert!(text.contains(">time</text>"));
        // an ac plot has two panels, so two of each axis, `freq` titling both x axes
        let row = |f: f64, m: f64, p: f64| vec![Cell::Real(f), Cell::Real(m), Cell::Real(p)];
        let table = Table { columns: vec!["freq".into(), "mag(T.o)".into(), "phase(T.o)".into()], rows: vec![row(1e3, 1.0, 0.0), row(1e5, 0.5, -45.0), row(1e7, 0.1, -90.0)] };
        let ac = SvgFixture::svg(&table, &Analysis::Ac { from: 1e3, to: 1e7, grid: Grid::PerDecade(1) });
        assert_eq!(ac.matches("<rect class=\"panel\"").count(), 2);
        assert_eq!(ac.matches("class=\"axis-title x\"").count(), 2);
        assert_eq!(ac.matches(">freq</text>").count(), 2);
        assert!(ac.contains(">magnitude (dB)</text>") && ac.contains(">phase (degrees)</text>"));
    }

    #[test]
    fn the_same_plot_is_the_same_bytes() {
        // CLI-22 AC 10
        assert_eq!(SvgFixture::svg(&SvgFixture::two_signals(), &SvgFixture::transient()), SvgFixture::svg(&SvgFixture::two_signals(), &SvgFixture::transient()));
        // a name with markup is escaped, never written raw
        let rows = vec![vec![Cell::Real(0.0), Cell::Real(1.0)], vec![Cell::Real(1.0), Cell::Real(2.0)]];
        let table = Table { columns: vec!["time".into(), "a<b&\"c\"".into()], rows };
        let text = SvgFixture::svg(&table, &SvgFixture::transient());
        assert!(text.contains("data-signal=\"a&lt;b&amp;&quot;c&quot;\""), "{text}");
    }
}
