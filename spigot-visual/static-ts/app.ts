// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

import type { Cell } from "./bucket-spigot-bindings/Cell.ts";
import type { NodeDetails } from "./bucket-spigot-bindings/NodeDetails.ts";
import type { TableView } from "./bucket-spigot-bindings/TableView.ts";
import type { Van } from "./van-1.5.5.d.ts";
import van from "./van-1.5.5.js";

import { REALISTIC_TABLE_JSON } from "./sample-input.js";

// CSS styles for the network visualization
const STYLES = `
  .network-container {
    padding: 20px;
    font-family: monospace;
    background: #f8f9fa;
    border-radius: 8px;
    margin: 10px 0;
  }

  .network-row {
    display: flex;
    align-items: center;
    margin: 8px 0;
    position: relative;
  }

  .node {
    border-radius: 6px;
    padding: 8px 12px;
    margin: 2px;
    cursor: pointer;
    transition: all 0.2s ease;
    position: relative;
    border: 2px solid transparent;
    min-width: 60px;
    text-align: center;
    font-size: 12px;
  }

  .node:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 8px rgba(0,0,0,0.2);
  }

  /* Node types */
  .node-spigot {
    background: #e74c3c;
    color: white;
    font-weight: bold;
    border-color: #c0392b;
  }

  .node-joint {
    background: #3498db;
    color: white;
    border-color: #2980b9;
  }

  .node-bucket {
    background: #2ecc71;
    color: white;
    border-color: #27ae60;
  }

  /* Active/Inactive states */
  .node-inactive {
    opacity: 0.6;
    filter: grayscale(0.3);
  }

  /* Connection lines */
  .connection-line {
    position: absolute;
    background: #34495e;
    pointer-events: none;
  }

  .connection-horizontal {
    height: 2px;
  }

  .connection-vertical {
    width: 2px;
  }

  /* Hover tooltip */
  .tooltip {
    position: absolute;
    background: #2c3e50;
    color: white;
    padding: 8px 12px;
    border-radius: 4px;
    font-size: 11px;
    z-index: 1000;
    pointer-events: none;
    white-space: nowrap;
    box-shadow: 0 2px 8px rgba(0,0,0,0.3);
  }

  /* Controls */
  .prototype-controls {
    margin: 20px 0;
    padding: 15px;
    background: #ecf0f1;
    border-radius: 6px;
  }

  .control-button {
    margin: 5px;
    padding: 8px 16px;
    background: #3498db;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .control-button:hover {
    background: #2980b9;
  }

  .control-input {
    margin: 5px;
    padding: 6px 10px;
    border: 1px solid #bdc3c7;
    border-radius: 4px;
  }
`;

function injectStyles() {
  if (!document.getElementById("network-styles")) {
    const styleEl = document.createElement("style");
    styleEl.id = "network-styles";
    styleEl.textContent = STYLES;
    document.head.appendChild(styleEl);
  }
}

function getNodeType(cell: Cell): "spigot" | "joint" | "bucket" | "empty" {
  if (!cell.node) return "empty";
  if (cell.node.path === ".") return "spigot";
  if ("Joint" in cell.node.kind || "JointAbbrev" in cell.node.kind)
    return "joint";
  if ("Bucket" in cell.node.kind) return "bucket";
  return "empty";
}

function formatNodeInfo(node: NodeDetails): string {
  const kindInfo =
    "Joint" in node.kind
      ? `Joint (${node.kind.Joint.child_count} children)`
      : "JointAbbrev" in node.kind
        ? `Joint Abbrev (${node.kind.JointAbbrev.child_count} children)`
        : "Bucket" in node.kind
          ? `Bucket (${node.kind.Bucket.item_count} items)`
          : "Unknown";

  return [
    `Path: ${node.path}`,
    `Type: ${kindInfo}`,
    `Order: ${node.order_type}`,
    `Weight: ${node.weight ?? "default"}`,
    `Status: ${node.active ? "Active" : "Inactive"}`,
  ].join(" | ");
}

function createTooltip(info: string): HTMLElement {
  const { div } = van.tags;
  return div({ class: "tooltip", style: "display: none;" }, info);
}

function showTooltip(tooltip: HTMLElement, event: MouseEvent) {
  tooltip.style.display = "block";
  tooltip.style.left = `${event.pageX + 10}px`;
  tooltip.style.top = `${event.pageY - 30}px`;
}

function hideTooltip(tooltip: HTMLElement) {
  tooltip.style.display = "none";
}

function renderHtmlCssNetwork(table: TableView): HTMLElement {
  const { div } = van.tags;

  const container = div({ class: "network-container" });
  const tooltipContainer = div({
    /* this breaks the mouse coordinate location logic
    style: "position: relative;"
    */
  });

  // Process each row
  for (const [rowIndex, row] of table.rows.entries()) {
    const rowEl = div({ class: "network-row" });

    for (const cell of row) {
      const nodeType = getNodeType(cell);

      if (nodeType !== "empty") {
        const nodeClasses = [
          "node",
          `node-${nodeType}`,
          cell.node && !cell.node.active ? "node-inactive" : "",
        ]
          .filter(Boolean)
          .join(" ");

        const nodeText =
          nodeType === "spigot"
            ? "SPIGOT"
            : nodeType === "joint"
              ? `J${cell.node?.weight ?? ""}`
              : nodeType === "bucket"
                ? `B${cell.node?.weight ?? ""}`
                : "";

        const nodeEl = div(
          {
            class: nodeClasses,
            style: `margin-left: ${cell.position * 20}px;`,
          },
          nodeText,
        );

        if (cell.node) {
          const tooltip = createTooltip(formatNodeInfo(cell.node));
          tooltipContainer.appendChild(tooltip);

          nodeEl.addEventListener("mouseenter", (e) =>
            showTooltip(tooltip, e as MouseEvent),
          );
          nodeEl.addEventListener("mouseleave", () => hideTooltip(tooltip));
          nodeEl.addEventListener("mousemove", (e) =>
            showTooltip(tooltip, e as MouseEvent),
          );
        }

        rowEl.appendChild(nodeEl);
      }
    }

    container.appendChild(rowEl);
  }

  container.appendChild(tooltipContainer);
  return container;
}

function createPlaceholderEditControls(): HTMLElement {
  const { div, button, input, select, option } = van.tags;

  return div(
    { class: "prototype-controls" },
    div("Placeholder Edit Controls:"),
    button({ class: "control-button" }, "Add Bucket"),
    button({ class: "control-button" }, "Add Joint"),
    button({ class: "control-button" }, "Delete Selected"),
    input({ class: "control-input", placeholder: "Node weight..." }),
    select(
      { class: "control-input" },
      option("InOrder"),
      option("Random"),
      option("Shuffle"),
    ),
    input({ class: "control-input", placeholder: "Filter strings..." }),
  );
}

function createPlayerPlaceholder(): HTMLElement {
  const { div, button } = van.tags;

  return div(
    {
      class: "prototype-controls",
      style: "background: #d5dbdb; border-left: 4px solid #e74c3c;",
    },
    div("Player UI Placeholder:"),
    div(
      { style: "display: flex; align-items: center; gap: 10px;" },
      div(
        {
          style:
            "width: 40px; height: 40px; background: #95a5a6; border-radius: 4px; display: flex; align-items: center; justify-content: center;",
        },
        "♪",
      ),
      div(
        { style: "flex: 1;" },
        div({ style: "font-weight: bold;" }, "Track Name"),
        div({ style: "font-size: 11px; color: #7f8c8d;" }, "Artist - Album"),
      ),
      button({ class: "control-button", style: "background: #e74c3c;" }, "⏮"),
      button({ class: "control-button", style: "background: #e74c3c;" }, "⏸"),
      button({ class: "control-button", style: "background: #e74c3c;" }, "⏭"),
    ),
  );
}

function renderSvgNetwork(table: TableView): HTMLElement {
  const { div } = van.tags;
  const { svg, g, rect, text } = van.tags("http://www.w3.org/2000/svg");

  const svgWidth = Math.max(600, table.total_width * 40 + 100);
  const svgHeight = Math.max(400, table.rows.length * 60 + 100);

  const svgContainer = div({
    style:
      "background: #f8f9fa; border-radius: 8px; padding: 20px; margin: 10px 0;",
  });
  const tooltipContainer = div({
    /* this breaks the mouse coordinate location logic
    style: "position: relative;"
    */
  });

  const svgEl = svg({
    width: svgWidth,
    height: svgHeight,
    viewBox: `0 0 ${svgWidth} ${svgHeight}`,
    style: "border: 1px solid #bdc3c7; border-radius: 4px; background: white;",
  });

  // Create connection and node groups
  const connectionsGroup = g({ id: "connections" });
  const nodesGroup = g({ id: "nodes" });

  for (const [rowIndex, row] of table.rows.entries()) {
    const y = 50 + rowIndex * 60;

    for (const cell of row) {
      const nodeType = getNodeType(cell);

      if (nodeType !== "empty") {
        const x = 50 + cell.position * 40;
        const width = Math.max(60, cell.display_width * 30);

        // Node background color
        const color =
          nodeType === "spigot"
            ? "#e74c3c"
            : nodeType === "joint"
              ? "#3498db"
              : "#2ecc71";

        const opacity = cell.node && !cell.node.active ? "0.6" : "1.0";

        // Create node rectangle
        const nodeRect = rect({
          x: x - width / 2,
          y: y - 20,
          width: width,
          height: 40,
          fill: color,
          stroke: "#2c3e50",
          "stroke-width": "2",
          rx: "6",
          ry: "6",
          opacity: opacity,
          style: "cursor: pointer; transition: all 0.2s ease;",
        });

        // Create node text
        const nodeText =
          nodeType === "spigot"
            ? "SPIGOT"
            : nodeType === "joint"
              ? `J${cell.node?.weight ?? ""}`
              : nodeType === "bucket"
                ? `B${cell.node?.weight ?? ""}`
                : "";

        const textEl = text(
          {
            x: x,
            y: y + 5,
            fill: "white",
            "text-anchor": "middle",
            "font-family": "monospace",
            "font-size": "12",
            "font-weight": nodeType === "spigot" ? "bold" : "normal",
            style: "pointer-events: none;",
          },
          nodeText,
        );

        // Connection lines to parent
        if (rowIndex > 0 && cell.parent_position !== cell.position) {
          const parentX = 50 + cell.parent_position * 40;
          const parentY = 50 + (rowIndex - 1) * 60;

          // Vertical line from parent
          const verticalLine = rect({
            x: parentX - 1,
            y: parentY + 20,
            width: 2,
            height: 30,
            fill: "#34495e",
          });

          // Horizontal line to child
          const horizontalLine = rect({
            x: Math.min(parentX, x) - 1,
            y: y - 21,
            width: Math.abs(x - parentX) + 2,
            height: 2,
            fill: "#34495e",
          });

          connectionsGroup.appendChild(verticalLine);
          connectionsGroup.appendChild(horizontalLine);
        }

        nodesGroup.appendChild(nodeRect);
        nodesGroup.appendChild(textEl);

        // Add hover functionality
        if (cell.node) {
          const tooltip = createTooltip(formatNodeInfo(cell.node));
          tooltipContainer.appendChild(tooltip);

          nodeRect.addEventListener("mouseenter", (e: Event) =>
            showTooltip(tooltip, e as MouseEvent),
          );
          nodeRect.addEventListener("mouseleave", () => hideTooltip(tooltip));
          nodeRect.addEventListener("mousemove", (e: Event) =>
            showTooltip(tooltip, e as MouseEvent),
          );
        }
      }
    }
  }

  svgEl.appendChild(connectionsGroup);
  svgEl.appendChild(nodesGroup);
  svgContainer.appendChild(svgEl);
  svgContainer.appendChild(tooltipContainer);

  return svgContainer;
}

function renderPrototype(): HTMLElement {
  const { div, h3, button, input, select, option } = van.tags;

  // Parse the realistic sample data
  const table: TableView = JSON.parse(REALISTIC_TABLE_JSON);

  let currentRenderer: "html" | "svg" = "html";

  const container = div();
  const visualizationContainer = div();

  const inputEl = input({
    type: "text",
    value: REALISTIC_TABLE_JSON,
    style:
      "width: 100%; margin: 10px 0; padding: 5px; font-family: monospace; font-size: 10px;",
  });

  const rendererSelect = select(
    {
      class: "control-input",
      style: "margin: 10px 5px;",
    },
    option({ value: "html" }, "HTML/CSS"),
    option({ value: "svg" }, "SVG"),
  );

  function updateVisualization() {
    try {
      const newTable: TableView = JSON.parse(inputEl.value);
      let networkViz: HTMLElement;

      switch (currentRenderer) {
        case "html":
          networkViz = renderHtmlCssNetwork(newTable);
          break;
        case "svg":
          networkViz = renderSvgNetwork(newTable);
          break;
        default:
          networkViz = renderHtmlCssNetwork(newTable);
      }

      visualizationContainer.innerHTML = "";
      visualizationContainer.appendChild(networkViz);
    } catch (e) {
      console.error("Invalid JSON:", e);
    }
  }

  rendererSelect.addEventListener("change", (e) => {
    currentRenderer = (e.target as HTMLSelectElement).value as "html" | "svg";
    updateVisualization();
  });

  container.appendChild(
    div(
      h3("Multi-Prototype Bucket-Spigot Network Visualizer"),
      createPlayerPlaceholder(),
      div(
        {
          style:
            "display: flex; align-items: center; gap: 10px; margin: 10px 0;",
        },
        div("Renderer:"),
        rendererSelect,
        button(
          {
            onclick: updateVisualization,
            class: "control-button",
          },
          "Update Visualization",
        ),
      ),
      div("JSON Input:"),
      inputEl,
      visualizationContainer,
      createPlaceholderEditControls(),
    ),
  );

  // Initial render
  updateVisualization();

  return container;
}

function set_content(container: HTMLElement, content: HTMLElement) {
  container.innerHTML = "";
  container.appendChild(content);
}

window.onload = async (): Promise<void> => {
  injectStyles();

  const content = document.getElementById("content");
  if (content != null) {
    set_content(content, renderPrototype());
  }
};
