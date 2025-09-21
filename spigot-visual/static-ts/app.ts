// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

import type { Van } from "./van-1.5.5.d.ts";
import van from "./van-1.5.5.js";

async function render_spigot(container: HTMLElement) {
  const { div, p } = van.tags;

  container.innerHTML = "";
  container.appendChild(div(p("Dynamic content from app.ts!")));
}

window.onload = async (): Promise<void> => {
  const content = document.getElementById("content");
  if (content != null) {
    await render_spigot(content);
  }
};
