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

  .network-container {
    display: flex;
    align-items: flex-start;
    gap: 20px;
    position: relative;
  }

  .network-column {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    position: relative;
    z-index: 2;
  }

  .connections-svg {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
    z-index: 1;
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

  const outerContainer = div({ class: "network-container" });
  const tooltipContainer = div({
    /* this breaks the mouse coordinate location logic
    style: "position: relative;"
    */
  });

  // Group cells by position (column) instead of row
  const columnMap = new Map<number, Array<{ cell: Cell; rowIndex: number }>>();

  for (const [rowIndex, row] of table.rows.entries()) {
    for (const cell of row) {
      const nodeType = getNodeType(cell);
      if (nodeType !== "empty") {
        const column = columnMap.get(cell.position);
        if (column) {
          column.push({ cell, rowIndex });
        } else {
          columnMap.set(cell.position, [{ cell, rowIndex }]);
        }
      }
    }
  }

  // Sort columns by position
  const sortedColumns = Array.from(columnMap.entries()).sort(
    ([a], [b]) => a - b,
  );

  for (const [position, cellsInColumn] of sortedColumns) {
    const columnEl = div({ class: "network-column" });

    // Sort cells in column by row index
    cellsInColumn.sort((a, b) => a.rowIndex - b.rowIndex);

    for (const { cell } of cellsInColumn) {
      const nodeType = getNodeType(cell);
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

      columnEl.appendChild(nodeEl);
    }

    outerContainer.appendChild(columnEl);
  }

  outerContainer.appendChild(tooltipContainer);
  return outerContainer;
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
  const { svg, g, rect, text, line, circle } = van.tags(
    "http://www.w3.org/2000/svg",
  );

  // Layout constants (similar to reference implementation)
  const CELL_HEIGHT = 50;
  const CELL_WIDTH = 100;
  const CELL_HEIGHT_PAD = 5;
  const CELL_WIDTH_PAD = 20;
  const CELL_X_STRIDE = CELL_WIDTH + CELL_WIDTH_PAD;
  const CELL_Y_STRIDE = CELL_HEIGHT + CELL_HEIGHT_PAD;

  // Calculate canvas dimensions
  const rowCount = table.rows.length;
  const canvasWidth = CELL_X_STRIDE * rowCount + 100;
  const canvasHeight = CELL_Y_STRIDE * table.total_width + 100;

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
    width: canvasWidth,
    height: canvasHeight,
    viewBox: `0 0 ${canvasWidth} ${canvasHeight}`,
    style: "border: 1px solid #bdc3c7; border-radius: 4px; background: white;",
  });

  // Create connection and node groups
  const connectionsGroup = g({ id: "connections" });
  const nodesGroup = g({ id: "nodes" });

  // Map to store node positions for connection drawing
  const nodePositions = new Map<string, { x: number; y: number }>();

  // Process each row (represents depth in tree, left to right)
  for (const [rowIndex, row] of table.rows.entries()) {
    let y = 0; // vertical position within this row

    for (const cell of row) {
      const nodeType = getNodeType(cell);

      if (nodeType !== "empty" && cell.node) {
        // X position: based on row index (depth in tree)
        const x = 50 + rowIndex * CELL_X_STRIDE + CELL_X_STRIDE / 2;

        // Y position: based on vertical position in this row
        // Use display_width to determine height span
        const cellY =
          50 + y * CELL_Y_STRIDE + (CELL_Y_STRIDE * cell.display_width) / 2;

        // Store position for connection drawing
        nodePositions.set(cell.node.path, { x, y: cellY });

        // Node background color
        const color =
          nodeType === "spigot"
            ? "#e74c3c"
            : nodeType === "joint"
              ? "#3498db"
              : "#2ecc71";

        const opacity = !cell.node.active ? "0.6" : "1.0";

        // Create node rectangle with height based on display_width
        const nodeHeight = CELL_Y_STRIDE * cell.display_width - CELL_HEIGHT_PAD;
        const nodeRect = rect({
          x: x - CELL_WIDTH / 2,
          y: cellY - nodeHeight / 2,
          width: CELL_WIDTH,
          height: nodeHeight,
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
              ? `J${cell.node.weight ?? ""}`
              : nodeType === "bucket"
                ? `B${cell.node.weight ?? ""}`
                : "";

        const textEl = text(
          {
            x: x,
            y: cellY + 5,
            fill: "white",
            "text-anchor": "middle",
            "font-family": "monospace",
            "font-size": "12",
            "font-weight": nodeType === "spigot" ? "bold" : "normal",
            style: "pointer-events: none;",
          },
          nodeText,
        );

        nodesGroup.appendChild(nodeRect);
        nodesGroup.appendChild(textEl);

        // Add hover functionality
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

      // Move y position by the display width of this cell
      y += cell.display_width;
    }
  }

  // Add root convergence point
  const rootX = 20;
  const rootY = canvasHeight / 2;

  // Draw connections after all nodes are positioned
  for (const [rowIndex, row] of table.rows.entries()) {
    for (const cell of row) {
      if (cell.node && cell.node.path !== ".") {
        const childPos = nodePositions.get(cell.node.path);
        // Find parent path by removing last segment
        const parentPath =
          cell.node.path.substring(0, cell.node.path.lastIndexOf(".")) || ".";

        if (childPos) {
          let parentPos = nodePositions.get(parentPath);

          // If parent is root (path "."), use root convergence point
          if (parentPath === ".") {
            parentPos = { x: rootX, y: rootY };
          }

          if (parentPos) {
            // Direct line from parent to child
            const connectionLine = line({
              x1: parentPos.x,
              y1: parentPos.y,
              x2: childPos.x,
              y2: childPos.y,
              stroke: "#34495e",
              "stroke-width": "2",
            });

            connectionsGroup.appendChild(connectionLine);
          }
        }
      }
    }
  }

  // Add root convergence indicator (small dot)
  const rootIndicator = circle({
    cx: rootX,
    cy: rootY,
    r: 4,
    fill: "#e74c3c",
    stroke: "#c0392b",
    "stroke-width": "2",
    opacity: "0.8",
  });

  nodesGroup.appendChild(rootIndicator);

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
