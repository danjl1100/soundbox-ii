// soundbox-ii/simple-bucket-spigot-html Prototype usecase for `bucket-spigot`
// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Prototype HTML viewer for `bucket-spigot`

use bucket_spigot::clap::clap_crate::{self as clap, Parser as _};
use bucket_spigot::clap::ArgBounds;
use bucket_spigot::view::TableView;
use bucket_spigot::{clap::ModifyCmd, view::TableParams, Network};

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

        add-bucket .4
        add-joint .4

        add-bucket .4.1
        add-bucket .4.1

        set-weight .0 0
        set-weight .0.1 50
        set-weight .1 2
        set-weight .2 3
        set-weight .3 4
        ";

    let mut app = App::<String, String>::new();
    app.update_for_commands_str(script)?;

    let params = TableParams::default();
    let html = app.render_view_html(params)?;
    println!("{html}");

    Ok(())
}

struct App<T, U> {
    network: Network<T, U>,
}
impl<T, U> App<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    fn new() -> Self {
        Self {
            network: Network::default(),
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
        Self::write_view_html(&view, &mut html)?;

        Ok(html)
    }
    fn write_view_html(table: &TableView, w: &mut impl std::fmt::Write) -> eyre::Result<()> {
        writeln!(
            w,
            "<html><head><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\" /></head>
             <body><table>"
        )?;
        for row in table.get_rows() {
            writeln!(w, "\t<tr>")?;
            for cell in row.get_cells() {
                let colspan = cell.get_display_width();
                if let Some(node) = cell.get_node() {
                    writeln!(w, "\t\t<td colspan=\"{colspan}\"><div class=\"cell\">")?;
                    writeln!(w, "\t\t\t<span><i class=\"arrow start\"></i></span>")?;
                    writeln!(w, "\t\t\t<div>{node}</div>")?;
                    writeln!(w, "\t\t\t<span><i class=\"arrow end\"></i></span>")?;
                    writeln!(w, "\t\t</div></td>")?;
                } else {
                    writeln!(w, "\t\t<td colspan=\"{colspan}\"></td>")?;
                }
            }
            writeln!(w, "\t</tr>")?;
        }
        writeln!(w, "</table></body></html>")?;
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
