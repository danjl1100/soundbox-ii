// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

import type { TableView } from "./bucket-spigot-bindings/TableView.ts";
import type { Van } from "./van-1.5.5.d.ts";
import van from "./van-1.5.5.js";

function use_table(table: TableView) {
  console.log(table.rows);
  console.log(table.total_width);
  console.log(table.rows[0][0].position);
}

async function render_spigot(container: HTMLElement, input_str: string | null) {
  const { div, p, input, button } = van.tags;

  if (input_str != null) {
    // proof that typescript allows JSON.parse into a specific type
    use_table(JSON.parse(input_str));
  }

  const text_json = input({
    type: "text",
    value: '{"rows": [[{"position":5}]], "total_width":20}',
  });

  container.innerHTML = "";
  container.appendChild(
    div(
      p("Dynamic content from app.ts!"),
      div(
        text_json,
        button(
          { onclick: () => use_table(JSON.parse(text_json.value)) },
          "Console log test",
        ),
      ),
    ),
  );
}

window.onload = async (): Promise<void> => {
  const content = document.getElementById("content");
  if (content != null) {
    await render_spigot(content, null);
  }
};
