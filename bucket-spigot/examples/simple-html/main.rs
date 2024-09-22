// soundbox-ii/bucket-spigot/simple-html Prototype view for `bucket-spigot`
// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Prototype HTML viewer for `bucket-spigot`

use bucket_spigot::clap::clap_crate::{self as clap, Parser as _};
use bucket_spigot::clap::ArgBounds;
use bucket_spigot::view::TableView;
use bucket_spigot::{clap::ModifyCmd, view::TableParams, Network};

/// helper for writing SVG elements, think `dbg!`
macro_rules! elem_write {
    ($dest:expr,  < $name:ident : $($var:ident)* />) => {{
        (|| {
            write!($dest, "<{name}", name = stringify!($name))?;
            $(
                write!($dest, " {name}=\"{value}\"", name = stringify!($var), value=$var)?;
            )+
            write!($dest, " />")
        })()
    }};
}
macro_rules! elem_writeln {
    ($dest:expr,  < $name:ident : $($var:ident)* />) => {{
        (|| {
            write!($dest, "\t")?;
            elem_write!($dest, < $name : $($var)* />)?;
            writeln!($dest)
        })()
    }};
}

// NOTE: only runs via `cargo test --examples`
#[test]
fn write_elem() {
    use std::fmt::Write as _;

    let mut s = String::new();
    let alpha = 5;
    let beta = 7;
    elem_write!(&mut s, < hello: alpha beta />).unwrap();
    assert_eq!(s, "<hello alpha=\"5\" beta=\"7\" />");

    s.clear();
    elem_writeln!(&mut s, < hello: alpha beta />).unwrap();
    assert_eq!(s, "\t<hello alpha=\"5\" beta=\"7\" />\n");
}

#[derive(clap::Parser)]
struct ExecArgs {
    #[clap(long)]
    render_mode: RenderMode,
}

#[derive(clap::ValueEnum, Clone, Copy, Default, Debug, PartialEq, Eq)]
enum RenderMode {
    Table,
    #[default]
    Svg,
}

fn main() -> eyre::Result<()> {
    // TODO script from user input (e.g. interactive prompt, file, or web input)
    // let script = "
    //     add-joint .
    //     add-bucket .0
    //     add-bucket .0

    //     add-joint .
    //     add-joint .1
    //     add-bucket .1

    //     add-joint .
    //     ";
    let script = "
        add-joint .
        add-bucket .
        add-bucket .
        add-bucket .
        add-joint .

        add-joint .0
        add-bucket .0

        add-bucket .0.0
        add-bucket .0.0
        add-bucket .0.0
        add-bucket .0.0
        add-bucket .0.0

        add-bucket .4
        add-joint .4
        add-bucket .4

        add-bucket .4.1
        add-bucket .4.1

        set-weight .0 0
        set-weight .0.1 50
        set-weight .1 2
        set-weight .2 3
        set-weight .3 4
        ";

    let args = ExecArgs::parse();

    let mut app = App::<String, String>::new(args.render_mode);
    app.update_for_commands_str(script)?;

    let params = TableParams::default();
    let html = app.render_view_html(params)?;
    println!("{html}");

    Ok(())
}

const DOCTYPE_HTML: &str = "<!DOCTYPE html>";

struct App<T, U> {
    network: Network<T, U>,
    render_mode: RenderMode,
}
impl<T, U> App<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    fn new(render_mode: RenderMode) -> Self {
        Self {
            network: Network::default(),
            render_mode,
        }
    }
    fn update_for_commands_str(&mut self, script: &str) -> eyre::Result<()> {
        for line in script.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let cmd = Command::try_parse_from(line.split_whitespace())?;
            let modify_cmd = cmd.modify_cmd.into();
            self.network.modify(modify_cmd)?;
        }
        Ok(())
    }
    fn render_view_html(&self, params: TableParams<'_>) -> eyre::Result<String> {
        let mut html = String::new();

        let view = self.network.view_table(params)?;
        self.write_view_html(&view, &mut html)?;

        Ok(html)
    }
    fn write_view_html(&self, table: &TableView, w: &mut impl std::fmt::Write) -> eyre::Result<()> {
        match self.render_mode {
            RenderMode::Table => Self::write_view_html_table(table, w),
            RenderMode::Svg => Self::write_view_html_svg(table, w),
        }
    }
    fn write_view_html_table(table: &TableView, w: &mut impl std::fmt::Write) -> eyre::Result<()> {
        writeln!(w, "{DOCTYPE_HTML}")?;
        writeln!(
            w,
            "<html><head><link rel=\"stylesheet\" type=\"text/css\" href=\"examples/simple-html/style.css\" /></head>
             <body><table>"
        )?;
        for row in table.get_rows() {
            writeln!(w, "\t<tr>")?;
            for cell in row.get_cells() {
                let colspan = cell.get_display_width();
                if let Some(node) = cell.get_node() {
                    let node_class = if node.is_bucket() { "bucket" } else { "joint" };
                    writeln!(w, "\t\t<td colspan=\"{colspan}\">")?;
                    writeln!(w, "\t\t\t<div class=\"cell {node_class}\">")?;
                    writeln!(w, "\t\t\t\t<span><i class=\"arrow start\"></i></span>")?;
                    writeln!(w, "\t\t\t\t<div class=\"node\">{node}</div>")?;
                    writeln!(w, "\t\t\t\t<span><i class=\"arrow end\"></i></span>")?;
                    writeln!(w, "\t\t\t</div>")?;
                    writeln!(w, "\t\t</div>")?;
                } else {
                    writeln!(w, "\t\t<td colspan=\"{colspan}\"></td>")?;
                }
            }
            writeln!(w, "\t</tr>")?;
        }
        writeln!(w, "</table></body></html>")?;
        Ok(())
    }
    /// Display the tree from left to right,
    ///
    /// # Example
    /// ```text
    /// .0   .0.0     .0.0.0
    /// .1   .1.0
    ///      .1.1
    /// .2   .2.0     .2.0.0
    ///               .2.0.1
    /// .3   .3.0
    /// ```
    fn write_view_html_svg(table: &TableView, w: &mut impl std::fmt::Write) -> eyre::Result<()> {
        const XMLNS: &str = "http://www.w3.org/2000/svg";
        const CELL_HEIGHT: u32 = 50;
        const CELL_WIDTH: u32 = 100;
        const CELL_HEIGHT_PAD: u32 = 5;
        const CELL_WIDTH_PAD: u32 = 20;
        const CELL_X_STRIDE: u32 = CELL_WIDTH + CELL_WIDTH_PAD;
        const CELL_Y_STRIDE: u32 = CELL_HEIGHT + CELL_HEIGHT_PAD;

        let Ok(row_count) = u32::try_from(table.get_rows().len()) else {
            eyre::bail!("table row count too large for u32")
        };
        let canvas_width = CELL_X_STRIDE * row_count;
        let canvas_height = CELL_Y_STRIDE * table.get_max_row_width();

        writeln!(w, "{DOCTYPE_HTML}")?;
        writeln!(w, "<html><head></head><body>")?;
        writeln!(
            w,
            "<svg viewBox=\"0 0 {canvas_width} {canvas_height}\" xmlns=\"{XMLNS}\">",
            // viewBox=\"-{half_stride_x} -{half_stride_y} ...
            // half_stride_x = f64::from(CELL_X_STRIDE) / 2.0,
            // half_stride_y = f64::from(CELL_Y_STRIDE) / 2.0,
        )?;
        writeln!(w, "<style>\n{}\n</style>", include_str!("style-svg.css"))?;

        for (x, row) in table.get_rows().iter().enumerate() {
            let mut y = 0;
            for cell in row.get_cells() {
                let colspan = cell.get_display_width();

                if let Some((node, colspan_less_one)) = cell.get_node().zip(colspan.checked_sub(1))
                {
                    let x_usize = x;
                    let x = u32::try_from(x).expect("row_count within bounds");

                    {
                        // source-level header
                        let node_class = if node.is_bucket() { "bucket" } else { "joint" };
                        let path = node.get_path();
                        writeln!(w, "\t <!-- {path} {node_class} -->")?;
                    }

                    {
                        // line to parent

                        let parent_y = cell.get_parent_position();
                        let parent_colspan = x_usize
                            .checked_sub(1)
                            .and_then(|prev_x| {
                                let prev_row = table.get_rows().get(prev_x)?;
                                let cells = prev_row.get_cells();
                                let parent_index = cells
                                    .binary_search_by_key(
                                        &parent_y,
                                        bucket_spigot::view::Cell::get_position,
                                    )
                                    .ok()?;
                                Some(cells[parent_index].get_display_width())
                            })
                            .unwrap_or(table.get_max_row_width());

                        // let parent_x = x.checked_sub(1);
                        // let x2 = parent_x.map_or(0.0, |parent_x| {
                        //     f64::from(CELL_X_STRIDE * parent_x) + (f64::from(CELL_WIDTH_PAD) * 0.5)
                        // });

                        let x1 = f64::from(CELL_X_STRIDE * x) + (f64::from(CELL_WIDTH_PAD) * 0.5);
                        let y1 = f64::from(CELL_Y_STRIDE * y)
                            + (f64::from(CELL_Y_STRIDE * colspan) * 0.5);
                        let x2 = (f64::from(CELL_X_STRIDE * x) - (f64::from(CELL_WIDTH_PAD) * 0.5))
                            .max(1.0);
                        let y2 = f64::from(CELL_Y_STRIDE * parent_y)
                            + (f64::from(CELL_Y_STRIDE * parent_colspan) * 0.5);
                        elem_writeln!(w, <line: x1 y1 x2 y2 />)?;
                    }

                    {
                        // rectangle

                        let x = f64::from(CELL_X_STRIDE * x) + (f64::from(CELL_WIDTH_PAD) * 0.5);
                        let y = f64::from(CELL_Y_STRIDE * y) + (f64::from(CELL_HEIGHT_PAD) * 0.5);
                        let width = CELL_WIDTH;
                        let height = CELL_Y_STRIDE * colspan_less_one + CELL_HEIGHT;
                        // writeln!(w, "\t<rect width=\"{width}\" height=\"{height}\" x=\"{rect_x}\" y=\"{rect_y}\"/>")?;
                        elem_writeln!(w, <rect: width height x y />)?;
                    }

                    {
                        // circle

                        let cx = f64::from(CELL_X_STRIDE * x) + (f64::from(CELL_X_STRIDE) * 0.5);
                        let cy = f64::from(CELL_Y_STRIDE * y)
                            + (f64::from(CELL_Y_STRIDE * colspan) * 0.5);
                        let r = f64::from(CELL_HEIGHT.min(CELL_WIDTH)) * 0.05;
                        // writeln!(w, "\t<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\"/>")?;
                        elem_writeln!(w, <circle: cx cy r />)?;
                    }
                }

                y += colspan;
            }
        }
        writeln!(w, "</svg></body></html>")?;
        Ok(())
    }
}

#[derive(clap::Parser)]
#[clap(no_binary_name = true)]
struct Command<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    #[clap(subcommand)]
    modify_cmd: ModifyCmd<T, U>,
}
