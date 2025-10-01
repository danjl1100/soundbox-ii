// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

import type { Cell } from "./bucket-spigot-bindings/Cell.ts";
import type { NodeDetails } from "./bucket-spigot-bindings/NodeDetails.ts";
import type { TableView } from "./bucket-spigot-bindings/TableView.ts";
import type { State, Van } from "./van-1.5.5.d.ts";
import van from "./van-1.5.5.js";

import { REALISTIC_TABLE_JSON } from "./sample-input.js";

// TODO remove unused
// function injectStyles() {
//   if (!document.getElementById("network-styles")) {
//     const styleEl = document.createElement("style");
//     styleEl.id = "network-styles";
//     styleEl.textContent = STYLES;
//     document.head.appendChild(styleEl);
//   }
// }

function getNodeType(cell: Cell): "joint" | "bucket" | "empty" {
  if (!cell.node) return "empty";
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
    `Status: ${node.active === true ? "Active" : "Inactive"}`,
  ].join(" | ");
}

function createTooltip(info: string): HTMLElement {
  const { div } = van.tags;
  return div({ class: "tooltip" }, info);
}

function showTooltip(tooltip: HTMLElement, event: MouseEvent) {
  tooltip.style.display = "block";
  tooltip.style.left = `${event.pageX + 10}px`;
  tooltip.style.top = `${event.pageY - 30}px`;
}

function hideTooltip(tooltip: HTMLElement) {
  tooltip.style.display = "none";
}

function createEditControls(
  selectedNodePath: State<string | null>,
): HTMLElement {
  const { a, div, button, input, select, option, span } = van.tags;

  const controlsClass = van.derive(() => {
    let classNames = "controls";
    if (selectedNodePath.val === null) {
      classNames += " hidden";
    }
    return classNames;
  });

  const x_button = a({
    style: "cursor: pointer;",
    onclick: () => {
      // clear selected node
      selectedNodePath.val = null;
    },
  });
  x_button.innerHTML = "&#x2715;"; // "&#10006;"; "&#x1F5D9;";

  return div(
    {
      class: controlsClass,
      style: "position: fixed; bottom: 0; right: 0; max-width: 50vw;",
    },
    div(
      { style: "display: flex;" },
      span(
        { style: "flex-grow: 1;" },
        van.derive(() => ` Edit ${selectedNodePath.val}`),
      ),
      x_button,
    ),
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

function createPlayer(): HTMLElement {
  const { div, button } = van.tags;

  return div(
    {
      class: "controls",
      style: "background: #d5dbdb; border-left: 4px solid #e74c3c;",
    },
    div("Player UI:"),
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

function renderSvgNetwork(
  table: TableView,
  selectedNodePath: State<string | null>,
): Array<Element> {
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
  const bucketWidthModifier = 2;
  const rowCount = table.rows.length;
  const canvasWidth =
    CELL_X_STRIDE * (rowCount - 1 + bucketWidthModifier) /*+ 100*/;
  const canvasHeight = CELL_Y_STRIDE * table.total_width /*+ 100*/;

  const tooltipContainer = div();

  // Create connection and node groups
  const connectionsGroup = g({ id: "connections" });
  const nodesGroup = g({ id: "nodes" });

  // Map to store node positions for connection drawing
  const nodePositions = new Map<string, { x: number; y: number }>();
  const getNodeOpacity = (node: NodeDetails) =>
    node.active === false ? "0.5" : "1.0";

  // Process each row (represents depth in tree, left to right)
  for (const [rowIndex, row] of table.rows.entries()) {
    let nextY = 0;

    for (const cell of row) {
      const y = nextY; // vertical position within this row

      // Move y position by the display width of this cell
      nextY += cell.display_width;

      const nodeType = getNodeType(cell);

      if (nodeType === "empty" || !cell.node) {
        continue;
      }
      const node: NodeDetails = cell.node;
      const nodeIsSelected: State<boolean> = van.derive(
        () => selectedNodePath.val === node.path,
      );

      // X position: based on row index (depth in tree)
      const x = rowIndex * CELL_X_STRIDE + CELL_X_STRIDE / 2;

      // Y position: based on vertical position in this row
      // Use display_width to determine height span
      const cellY =
        y * CELL_Y_STRIDE + (CELL_Y_STRIDE * cell.display_width) / 2;

      // Store position for connection drawing
      nodePositions.set(node.path, { x, y: cellY });

      // Node background color
      const color =
        nodeType === "joint"
          ? "#3498db"
          : nodeType === "bucket"
            ? "#2ecc71"
            : "";

      // Create node rectangle with height based on display_width
      const nodeHeight = CELL_HEIGHT; // CELL_Y_STRIDE * cell.display_width - CELL_HEIGHT_PAD;
      let nodeWidth = CELL_WIDTH;
      if (nodeType === "bucket") {
        nodeWidth = bucketWidthModifier * CELL_WIDTH;
      }
      const nodeRect = rect({
        x: x - CELL_WIDTH / 2,
        y: cellY - nodeHeight / 2,
        width: nodeWidth,
        height: nodeHeight,
        fill: color,
        stroke: van.derive(() =>
          nodeIsSelected.val === true ? /* "#159534" */ "#e2c912" : "#2c3e50",
        ),
        "stroke-width": van.derive(() =>
          nodeIsSelected.val === true ? "5" : "2",
        ),
        rx: "6",
        ry: "6",
        opacity: getNodeOpacity(node),
        style: "cursor: pointer; transition: all 0.2s ease;",
      });

      // Create node text
      const nodeText =
        nodeType === "joint" ? "Joint" : nodeType === "bucket" ? "Bucket" : "";

      const textEl = text(
        {
          x: x - CELL_WIDTH / 4,
          y: cellY + 5,
          fill: "white",
          "text-anchor": "left",
          "font-family": "monospace",
          "font-size": "12",
          style: "pointer-events: none;",
        },
        nodeText,
      );

      nodesGroup.appendChild(nodeRect);
      nodesGroup.appendChild(textEl);

      if (cell.node.weight) {
        const textWeight = text(
          {
            x: x - CELL_WIDTH / 2 + 5,
            y: cellY + 5,
            fill: "white",
            "text-anchor": "left",
            "font-family": "monospace",
            "font-size": "12",
            style: "pointer-events: none;",
          },
          cell.node.weight,
        );
        nodesGroup.appendChild(textWeight);
      }

      // Add hover functionality
      const tooltip = createTooltip(formatNodeInfo(cell.node));
      tooltipContainer.appendChild(tooltip);

      nodeRect.addEventListener("mousedown", (e: Event) => {
        const event = e as MouseEvent;
        if (event.buttons === 1) {
          selectedNodePath.val = node.path;
        }
      });
      nodeRect.addEventListener("mouseenter", (e: Event) =>
        showTooltip(tooltip, e as MouseEvent),
      );
      nodeRect.addEventListener("mouseleave", () => hideTooltip(tooltip));
      nodeRect.addEventListener("mousemove", (e: Event) =>
        showTooltip(tooltip, e as MouseEvent),
      );
    }
  }

  // Add root convergence point
  const rootX = -10;
  const rootY = canvasHeight / 2;

  // Draw connections after all nodes are positioned
  for (const [rowIndex, row] of table.rows.entries()) {
    for (const cell of row) {
      if (cell.node && cell.node.path !== ".") {
        const node = cell.node;

        const childPos = nodePositions.get(cell.node.path);
        // Find parent path by removing last segment
        const parentPath =
          cell.node.path.substring(0, cell.node.path.lastIndexOf(".")) || ".";

        if (childPos) {
          let parentPos = nodePositions.get(parentPath);

          // If parent is root (path "."), use root convergence point
          if (parentPath === ".") {
            parentPos = { x: rootX - CELL_WIDTH / 2, y: rootY };
          }

          if (parentPos) {
            // Direct line from parent to child
            const connectionLine = line({
              x1: parentPos.x + CELL_WIDTH / 2,
              y1: parentPos.y,
              x2: childPos.x - CELL_WIDTH / 2,
              y2: childPos.y,
              stroke: "#34495e",
              "stroke-width": "2",
              opacity: getNodeOpacity(node),
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

  const viewBoxX = rootX - 5;
  const viewBoxY = 0;
  const viewBoxWidth = Math.max(canvasWidth, 800);
  const viewBoxHeight = canvasHeight;

  const background = rect({
    x: viewBoxX,
    y: viewBoxY,
    width: viewBoxWidth,
    height: viewBoxHeight,
    opacity: 0.0,
  });
  background.addEventListener("mousedown", (e: Event) => {
    selectedNodePath.val = null;
  });

  nodesGroup.appendChild(rootIndicator);

  const svgEl = svg(
    {
      // width: canvasWidth,
      // height: canvasHeight,
      viewBox: `${viewBoxX} ${viewBoxY} ${viewBoxWidth} ${viewBoxHeight}`,
      style:
        "border: 1px solid #bdc3c7; border-radius: 4px; background: white; width: 100%;",
    },
    background,
    connectionsGroup,
    nodesGroup,
  );

  return [svgEl, tooltipContainer];
}

function render(container: HTMLElement): Array<HTMLElement> {
  const { div, h3, button, input, select, option } = van.tags;

  // Parse the realistic sample data
  const table: TableView = JSON.parse(REALISTIC_TABLE_JSON);

  const inputEl = input({
    type: "text",
    value: REALISTIC_TABLE_JSON,
    style:
      "width: 100%; margin: 10px 0; padding: 5px; font-family: monospace; font-size: 10px;",
  });

  const selectedNodePath: State<string | null> = van.state(null);

  const visualizationContainer = div({
    style: "width: 100%; height: 100%;",
  });
  function updateVisualization() {
    try {
      const newTable: TableView = JSON.parse(inputEl.value);
      const children: Array<Element> = [];
      children.push(createEditControls(selectedNodePath));
      children.push(...renderSvgNetwork(newTable, selectedNodePath));
      setContent(visualizationContainer, children);
    } catch (e) {
      console.error("Invalid JSON:", e);
    }
  }
  // Initial render
  updateVisualization();

  return [
    h3("Bucket-Spigot Network Visualizer"),
    createPlayer(),
    div("JSON Input:", inputEl),
    visualizationContainer,
  ];
}

function setContent(container: HTMLElement, content: Array<Element>) {
  container.innerHTML = "";
  for (const child of content) {
    container.appendChild(child);
  }
}

window.onload = async (): Promise<void> => {
  const content = document.getElementById("content");
  if (content != null) {
    setContent(content, render(content));
  }
};
