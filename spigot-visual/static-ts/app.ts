// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

async function render_spigot(container: HTMLElement) {
  container.innerHTML = '';

  const placeholder = document.createElement('p');
  placeholder.innerText = 'Dynamic content from app.ts!';
  container.appendChild(placeholder);
}

window.onload = async (): Promise<void> => {
  const content = document.getElementById('content');
  if (content != null) {
    await render_spigot(content);
  }
};
