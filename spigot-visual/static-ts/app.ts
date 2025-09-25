// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

import type { TableView } from "./bucket-spigot-bindings/TableView.ts";
import type { Van } from "./van-1.5.5.d.ts";
import van from "./van-1.5.5.js";

import { REALISTIC_TABLE_JSON } from "./sample-input.js";

function set_content(container: HTMLElement, content: HTMLElement) {
  container.innerHTML = "";
  container.appendChild(content);
}

function render_nodes(table: TableView) {
  const { div } = van.tags;

  console.log(table.rows);
  console.log(table.total_width);
  console.log(table.rows[0][0].position);

  return div(`TODO - render nodes width ${table.total_width}`);
}

async function render_ui(container: HTMLElement) {
  const { div, p, input, button } = van.tags;

  const text_json = input({
    type: "text",
    value: '{"rows": [[{"position":5}]], "total_width":20}',
  });

  const nodes_container = div("Click a button to render nodes");
  function fill_nodes_container(json_input: string) {
    set_content(nodes_container, render_nodes(JSON.parse(json_input)));
  }

  set_content(
    container,
    div(
      p("Dynamic content from app.ts!"),
      div(
        text_json,
        button(
          { onclick: () => fill_nodes_container(text_json.value) },
          "Render from input",
        ),
        button(
          { onclick: () => fill_nodes_container(REALISTIC_TABLE_JSON) },
          "Render realistic table",
        ),
      ),
      nodes_container,
    ),
  );
}

window.onload = async (): Promise<void> => {
  const content = document.getElementById("content");
  if (content != null) {
    await render_ui(content);
  }
};
