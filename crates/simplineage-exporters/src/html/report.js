/* SimpLineage offline interactive lineage graph — no external deps */
(function () {
  "use strict";

  const dataEl = document.getElementById("report-data");
  /** @type {any} */
  const DATA = JSON.parse(dataEl.textContent || "{}");

  const svg = document.getElementById("graph");
  const viewport = document.getElementById("viewport");
  const edgesG = document.getElementById("edges");
  const nodesG = document.getElementById("nodes");
  const canvasWrap = document.getElementById("canvas-wrap");

  const state = {
    scale: 1,
    tx: 20,
    ty: 20,
    selectedId: null,
    impact: null, // { subject, direction, up:Set, down:Set }
    search: "",
    kinds: new Set(),
    hideIsolated: false,
    relationsOnly: true,
    panning: false,
    panStart: null,
    /** True if the current pointer gesture moved enough to count as a pan. */
    panMoved: false,
  };

  const RELATION_KINDS = new Set(["table", "view", "materialized_view", "unknown"]);

  // Build adjacency
  const outs = new Map();
  const ins = new Map();
  (DATA.nodes || []).forEach((n) => {
    outs.set(n.id, []);
    ins.set(n.id, []);
  });
  (DATA.edges || []).forEach((e) => {
    if (!outs.has(e.from)) outs.set(e.from, []);
    if (!ins.has(e.to)) ins.set(e.to, []);
    outs.get(e.from).push(e.to);
    ins.get(e.to).push(e.from);
  });

  function initChrome() {
    const sub = document.getElementById("report-subtitle");
    const label = DATA.meta && DATA.meta.label ? DATA.meta.label : DATA.meta.snapshot_id;
    sub.textContent =
      label +
      " · " +
      (DATA.stats.graph_nodes || 0) +
      " nodes · " +
      (DATA.stats.graph_edges || 0) +
      " edges";

    const stats = document.getElementById("stats-list");
    const s = DATA.stats || {};
    const rows = [
      ["tables", s.tables],
      ["views", s.views],
      ["materialized_views", s.materialized_views],
      ["columns", s.columns],
      ["dependencies", s.dependencies],
      ["graph_nodes", s.graph_nodes],
    ];
    stats.innerHTML = rows
      .map(([k, v]) => "<dt>" + k + "</dt><dd>" + (v == null ? "0" : v) + "</dd>")
      .join("");

    // Kind filters
    const kinds = new Set((DATA.nodes || []).map((n) => n.kind));
    const box = document.getElementById("kind-filters");
    const preferred = ["table", "view", "materialized_view", "column", "schema", "database", "catalog", "unknown"];
    const ordered = preferred.filter((k) => kinds.has(k)).concat(
      [...kinds].filter((k) => !preferred.includes(k)).sort()
    );
    ordered.forEach((k) => {
      state.kinds.add(k);
      const id = "kind-" + k;
      const label = document.createElement("label");
      label.className = "check";
      label.innerHTML =
        '<input type="checkbox" id="' +
        id +
        '" data-kind="' +
        k +
        '" checked/> ' +
        k;
      box.appendChild(label);
      label.querySelector("input").addEventListener("change", (ev) => {
        if (ev.target.checked) state.kinds.add(k);
        else state.kinds.delete(k);
        applyVisibility();
      });
    });

    document.getElementById("hide-isolated").addEventListener("change", (e) => {
      state.hideIsolated = e.target.checked;
      applyVisibility();
    });
    document.getElementById("relations-only").addEventListener("change", (e) => {
      state.relationsOnly = e.target.checked;
      applyVisibility();
    });
    document.getElementById("search").addEventListener("input", (e) => {
      state.search = (e.target.value || "").trim().toLowerCase();
      updateFocusSummary();
      applyVisibility();
    });

    document.getElementById("btn-fit").addEventListener("click", fitView);
    document.getElementById("btn-zoom-in").addEventListener("click", () => zoomAt(1.2));
    document.getElementById("btn-zoom-out").addEventListener("click", () => zoomAt(1 / 1.2));
    document.getElementById("btn-theme").addEventListener("click", toggleTheme);
    document.getElementById("btn-export-svg").addEventListener("click", exportSvg);
    document.getElementById("btn-export-mermaid").addEventListener("click", exportMermaid);

    document.getElementById("btn-up").addEventListener("click", () => runImpact("upstream"));
    document.getElementById("btn-down").addEventListener("click", () => runImpact("downstream"));
    document.getElementById("btn-both").addEventListener("click", () => runImpact("both"));
    document.getElementById("btn-clear-impact").addEventListener("click", clearImpact);

    // Click empty canvas to clear selection + impact focus (search stays).
    svg.addEventListener("click", (e) => {
      const isNode = e.target.closest && e.target.closest(".node");
      if (isNode || state.panMoved) return;
      if (state.selectedId || state.impact) {
        clearSelection();
      }
    });

    setupPanZoom();
  }

  function toggleTheme() {
    const html = document.documentElement;
    const next = html.getAttribute("data-theme") === "dark" ? "light" : "dark";
    html.setAttribute("data-theme", next);
    try {
      localStorage.setItem("simplineage-theme", next);
    } catch (_) {}
  }

  function loadTheme() {
    try {
      const t = localStorage.getItem("simplineage-theme");
      if (t === "dark" || t === "light") document.documentElement.setAttribute("data-theme", t);
      else if (window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches) {
        document.documentElement.setAttribute("data-theme", "dark");
      }
    } catch (_) {}
  }

  function nodeById(id) {
    return (DATA.nodes || []).find((n) => n.id === id);
  }

  function isVisibleNode(n) {
    if (!state.kinds.has(n.kind)) return false;
    if (state.relationsOnly && !RELATION_KINDS.has(n.kind) && n.kind !== "unknown") return false;
    if (state.hideIsolated) {
      const o = (outs.get(n.id) || []).length;
      const i = (ins.get(n.id) || []).length;
      if (o + i === 0) return false;
    }
    return true;
  }

  /** Text match only (id / FQN / name / kind). Empty search → false. */
  function isSearchHit(n) {
    if (!state.search || !n) return false;
    const q = state.search;
    return (
      n.id.toLowerCase().includes(q) ||
      (n.fqn && n.fqn.toLowerCase().includes(q)) ||
      (n.name && n.name.toLowerCase().includes(q)) ||
      (n.kind && n.kind.toLowerCase().includes(q))
    );
  }

  function bfs(start, neighborFn) {
    const seen = new Set();
    const q = [start];
    while (q.length) {
      const u = q.shift();
      (neighborFn(u) || []).forEach((v) => {
        if (!seen.has(v) && v !== start) {
          seen.add(v);
          q.push(v);
        }
      });
    }
    return seen;
  }

  /**
   * Build lineage neighborhood for one or more seed node ids.
   * direction: "upstream" | "downstream" | "both"
   */
  function lineageSets(seedIds, direction) {
    const seeds = new Set(seedIds);
    const up = new Set();
    const down = new Set();
    seeds.forEach((seed) => {
      if (direction !== "downstream") {
        bfs(seed, (id) => ins.get(id) || []).forEach((x) => {
          if (!seeds.has(x)) up.add(x);
        });
      }
      if (direction !== "upstream") {
        bfs(seed, (id) => outs.get(id) || []).forEach((x) => {
          if (!seeds.has(x)) down.add(x);
        });
      }
    });
    return { seeds, up, down };
  }

  /**
   * Combined focus from impact selection and/or search.
   * Nodes/edges outside this set are dimmed when `active` is true.
   */
  function computeFocus() {
    const seeds = new Set();
    const up = new Set();
    const down = new Set();
    const matches = new Set();
    let active = false;

    if (state.impact) {
      active = true;
      seeds.add(state.impact.subject);
      state.impact.up.forEach((id) => up.add(id));
      state.impact.down.forEach((id) => down.add(id));
    }

    if (state.search) {
      const hits = (DATA.nodes || []).filter(isSearchHit);
      if (hits.length) {
        active = true;
        const lin = lineageSets(
          hits.map((n) => n.id),
          "both"
        );
        hits.forEach((n) => {
          matches.add(n.id);
          seeds.add(n.id);
        });
        lin.up.forEach((id) => {
          if (!seeds.has(id)) up.add(id);
        });
        lin.down.forEach((id) => {
          if (!seeds.has(id)) down.add(id);
        });
      } else {
        // Search text with no hits: dim everything visible.
        active = true;
      }
    }

    // Prefer seed membership over up/down when overlapping.
    up.forEach((id) => {
      if (seeds.has(id)) up.delete(id);
    });
    down.forEach((id) => {
      if (seeds.has(id)) down.delete(id);
    });

    function isFocused(id) {
      return seeds.has(id) || up.has(id) || down.has(id);
    }

    return { active, seeds, up, down, matches, isFocused };
  }

  function edgeFocusClass(from, to, focus) {
    if (!focus.active) return null;
    const { seeds, up, down, isFocused } = focus;
    if (!isFocused(from) || !isFocused(to)) return "dim";
    // Upstream path toward a seed
    if (up.has(from) && (up.has(to) || seeds.has(to))) return "impact-up";
    // Downstream path from a seed
    if (down.has(to) && (down.has(from) || seeds.has(from))) return "impact-down";
    // Edge between two seeds (e.g. multi-match search)
    if (seeds.has(from) && seeds.has(to)) return "impact-down";
    // Connected within neighborhood but not a clean path (still keep bright)
    return null;
  }

  function updateFocusSummary() {
    const el = document.getElementById("impact-summary");
    if (!el) return;
    const parts = [];
    if (state.impact) {
      const total = new Set([...state.impact.up, ...state.impact.down]).size;
      parts.push(
        state.impact.direction +
          " of selected: " +
          total +
          " related (↑" +
          state.impact.up.size +
          " ↓" +
          state.impact.down.size +
          ")"
      );
    }
    if (state.search) {
      const focus = computeFocus();
      const hitCount = focus.matches.size;
      const related = new Set(
        [...focus.seeds, ...focus.up, ...focus.down].filter((id) => !focus.matches.has(id))
      ).size;
      if (hitCount === 0) {
        parts.push('search "' + state.search + '": no matches');
      } else {
        parts.push(
          'search "' +
            state.search +
            '": ' +
            hitCount +
            " match" +
            (hitCount === 1 ? "" : "es") +
            " + " +
            related +
            " related (↑↓)"
        );
      }
    }
    el.textContent = parts.join(" · ");
  }

  function applyTransform() {
    viewport.setAttribute(
      "transform",
      "translate(" + state.tx + "," + state.ty + ") scale(" + state.scale + ")"
    );
  }

  function setupPanZoom() {
    let spacePan = false;
    svg.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const rect = svg.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
        zoomAtPoint(factor, mx, my);
      },
      { passive: false }
    );

    svg.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      // pan if background or middle
      const isNode = e.target.closest && e.target.closest(".node");
      if (isNode) return;
      state.panning = true;
      state.panMoved = false;
      svg.classList.add("panning");
      state.panStart = { x: e.clientX, y: e.clientY, tx: state.tx, ty: state.ty };
      svg.setPointerCapture(e.pointerId);
    });
    svg.addEventListener("pointermove", (e) => {
      if (!state.panning || !state.panStart) return;
      const dx = e.clientX - state.panStart.x;
      const dy = e.clientY - state.panStart.y;
      if (Math.abs(dx) > 3 || Math.abs(dy) > 3) state.panMoved = true;
      state.tx = state.panStart.tx + dx;
      state.ty = state.panStart.ty + dy;
      applyTransform();
    });
    svg.addEventListener("pointerup", (e) => {
      state.panning = false;
      state.panStart = null;
      svg.classList.remove("panning");
      try {
        svg.releasePointerCapture(e.pointerId);
      } catch (_) {}
    });
    void spacePan;
  }

  function zoomAt(factor) {
    const rect = svg.getBoundingClientRect();
    zoomAtPoint(factor, rect.width / 2, rect.height / 2);
  }

  function zoomAtPoint(factor, mx, my) {
    const prev = state.scale;
    const next = Math.min(4, Math.max(0.15, prev * factor));
    // keep point under cursor stable
    const wx = (mx - state.tx) / prev;
    const wy = (my - state.ty) / prev;
    state.scale = next;
    state.tx = mx - wx * next;
    state.ty = my - wy * next;
    applyTransform();
  }

  function fitView() {
    const visible = (DATA.nodes || []).filter(isVisibleNode);
    if (!visible.length) return;
    let minX = Infinity,
      minY = Infinity,
      maxX = -Infinity,
      maxY = -Infinity;
    visible.forEach((n) => {
      minX = Math.min(minX, n.x);
      minY = Math.min(minY, n.y);
      maxX = Math.max(maxX, n.x + 160);
      maxY = Math.max(maxY, n.y + 44);
    });
    const rect = svg.getBoundingClientRect();
    const pad = 40;
    const w = maxX - minX + pad * 2;
    const h = maxY - minY + pad * 2;
    const sx = rect.width / w;
    const sy = rect.height / h;
    state.scale = Math.min(1.5, Math.max(0.2, Math.min(sx, sy)));
    state.tx = (rect.width - w * state.scale) / 2 - (minX - pad) * state.scale;
    state.ty = (rect.height - h * state.scale) / 2 - (minY - pad) * state.scale;
    applyTransform();
  }

  function draw() {
    edgesG.innerHTML = "";
    nodesG.innerHTML = "";
    const nodes = DATA.nodes || [];
    const edges = DATA.edges || [];
    const byId = new Map(nodes.map((n) => [n.id, n]));

    edges.forEach((e) => {
      const a = byId.get(e.from);
      const b = byId.get(e.to);
      if (!a || !b) return;
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("class", "edge");
      path.dataset.from = e.from;
      path.dataset.to = e.to;
      path.dataset.id = e.id;
      path.setAttribute("marker-end", "url(#arrow)");
      // Node box is 160×44; attach to midpoints of facing sides.
      const x1 = a.x + 160;
      const y1 = a.y + 22;
      const x2 = b.x;
      const y2 = b.y + 22;
      const dx = Math.max(24, Math.abs(x2 - x1));
      // Cap horizontal pull so steep edges do not bow into huge S-curves.
      const pull = Math.min(80, dx * 0.45);
      const c1x = x1 + pull;
      const c2x = x2 - pull;
      path.setAttribute(
        "d",
        "M " + x1 + " " + y1 + " C " + c1x + " " + y1 + ", " + c2x + " " + y2 + ", " + x2 + " " + y2
      );
      edgesG.appendChild(path);
    });

    nodes.forEach((n) => {
      const g = document.createElementNS("http://www.w3.org/2000/svg", "g");
      g.setAttribute("class", "node kind-" + (n.kind || "unknown"));
      g.dataset.id = n.id;
      g.setAttribute("transform", "translate(" + n.x + "," + n.y + ")");

      const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      rect.setAttribute("width", "160");
      rect.setAttribute("height", "44");
      rect.setAttribute("rx", "8");
      rect.setAttribute("ry", "8");

      const kind = document.createElementNS("http://www.w3.org/2000/svg", "text");
      kind.setAttribute("class", "kind-label");
      kind.setAttribute("x", "10");
      kind.setAttribute("y", "14");
      kind.textContent = n.kind || "";

      const label = document.createElementNS("http://www.w3.org/2000/svg", "text");
      label.setAttribute("x", "10");
      label.setAttribute("y", "32");
      const text = n.name || n.id;
      label.textContent = text.length > 22 ? text.slice(0, 20) + "…" : text;

      g.appendChild(rect);
      g.appendChild(kind);
      g.appendChild(label);
      g.addEventListener("click", (ev) => {
        ev.stopPropagation();
        selectNode(n.id);
      });
      nodesG.appendChild(g);
    });

    applyVisibility();
    applyTransform();
  }

  function applyVisibility() {
    const visibleIds = new Set();
    (DATA.nodes || []).forEach((n) => {
      if (isVisibleNode(n)) visibleIds.add(n.id);
    });
    const focus = computeFocus();

    nodesG.querySelectorAll(".node").forEach((el) => {
      const n = nodeById(el.dataset.id);
      const id = el.dataset.id;
      const vis = n && visibleIds.has(id);
      el.style.display = vis ? "" : "none";
      el.classList.remove("dim", "match", "selected", "impact-subject", "impact-up", "impact-down");
      if (!vis) return;

      const focused = !focus.active || focus.isFocused(id);
      if (focus.active && !focused) {
        el.classList.add("dim");
        return;
      }
      if (state.selectedId === id) el.classList.add("selected");
      if (focus.matches.has(id)) el.classList.add("match");
      // Impact subject (click selection) takes gold; search-only seeds use match.
      if (state.impact && id === state.impact.subject) {
        el.classList.add("impact-subject");
      } else if (focus.seeds.has(id) && !focus.matches.has(id)) {
        el.classList.add("impact-subject");
      }
      if (focus.up.has(id)) el.classList.add("impact-up");
      if (focus.down.has(id)) el.classList.add("impact-down");
    });

    edgesG.querySelectorAll(".edge").forEach((el) => {
      const ok = visibleIds.has(el.dataset.from) && visibleIds.has(el.dataset.to);
      el.style.display = ok ? "" : "none";
      el.classList.remove("dim", "impact-up", "impact-down");
      if (!ok) return;
      const cls = edgeFocusClass(el.dataset.from, el.dataset.to, focus);
      if (cls) el.classList.add(cls);
    });
  }

  function setImpactButtons(hasSelection) {
    ["btn-up", "btn-down", "btn-both"].forEach((bid) => {
      document.getElementById(bid).disabled = !hasSelection;
    });
    // Clear resets click/impact focus (search box is cleared separately by the user).
    document.getElementById("btn-clear-impact").disabled = !state.impact && !hasSelection;
  }

  function fillDetailPanel(n) {
    const empty = document.getElementById("detail-empty");
    const body = document.getElementById("detail-body");
    if (!n) {
      empty.classList.remove("hidden");
      body.classList.add("hidden");
      return;
    }
    empty.classList.add("hidden");
    body.classList.remove("hidden");
    document.getElementById("d-kind").textContent = n.kind;
    document.getElementById("d-name").textContent = n.name || n.id;
    document.getElementById("d-fqn").textContent = n.fqn || "";
    document.getElementById("d-id").textContent = n.id;
    document.getElementById("d-desc").textContent = n.description || "No description.";

    const neigh = document.getElementById("d-neighbors");
    const up = ins.get(n.id) || [];
    const down = outs.get(n.id) || [];
    let html = "";
    if (up.length) {
      html += "<div class='muted'>Upstream (" + up.length + ")</div>";
      up.forEach((u) => {
        const nn = nodeById(u);
        html +=
          "<div class='chip' data-id='" +
          escapeAttr(u) +
          "'>↑ " +
          escapeHtml((nn && (nn.fqn || nn.name)) || u) +
          "</div>";
      });
    }
    if (down.length) {
      html += "<div class='muted' style='margin-top:0.5rem'>Downstream (" + down.length + ")</div>";
      down.forEach((u) => {
        const nn = nodeById(u);
        html +=
          "<div class='chip' data-id='" +
          escapeAttr(u) +
          "'>↓ " +
          escapeHtml((nn && (nn.fqn || nn.name)) || u) +
          "</div>";
      });
    }
    if (!up.length && !down.length) html = "<span class='muted'>No neighbors</span>";
    neigh.innerHTML = html;
    neigh.querySelectorAll(".chip").forEach((el) => {
      el.addEventListener("click", () => selectNode(el.dataset.id));
    });
  }

  /**
   * Select a node: show details and auto-focus its full upstream + downstream
   * (dim everything else). Upstream/Downstream/Both buttons narrow the focus.
   */
  function selectNode(id) {
    state.selectedId = id;
    const n = nodeById(id);
    setImpactButtons(!!n);
    fillDetailPanel(n);
    if (!n) {
      state.impact = null;
      updateFocusSummary();
      applyVisibility();
      return;
    }
    // Default: both directions on every selection.
    runImpact("both");
  }

  function runImpact(direction) {
    if (!state.selectedId) return;
    const subject = state.selectedId;
    const lin = lineageSets([subject], direction);
    state.impact = {
      subject: subject,
      direction: direction,
      up: lin.up,
      down: lin.down,
    };
    setImpactButtons(true);
    updateFocusSummary();
    applyVisibility();
  }

  /** Reset click selection + impact focus. Search query is left unchanged. */
  function clearSelection() {
    state.selectedId = null;
    state.impact = null;
    setImpactButtons(false);
    fillDetailPanel(null);
    updateFocusSummary();
    applyVisibility();
  }

  function clearImpact() {
    clearSelection();
  }

  function exportSvg() {
    const clone = svg.cloneNode(true);
    // Inline computed theme colors as best-effort for standalone SVG
    clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    const style = document.createElementNS("http://www.w3.org/2000/svg", "style");
    style.textContent =
      "text{font-family:system-ui,sans-serif} .edge{fill:none;stroke:#64748b;stroke-width:1.5} .arrow-head{fill:#64748b} .node rect{fill:#fff;stroke:#64748b;stroke-width:1.5} .node text{fill:#0f172a;font-size:12px} .kind-label{font-size:9px;fill:#64748b}";
    clone.insertBefore(style, clone.firstChild);
    const xml =
      '<?xml version="1.0" encoding="UTF-8"?>\n' + new XMLSerializer().serializeToString(clone);
    downloadBlob(xml, "simplineage-lineage.svg", "image/svg+xml");
  }

  function exportMermaid() {
    const lines = ["flowchart LR"];
    const visible = (DATA.nodes || []).filter(isVisibleNode);
    const ids = new Set(visible.map((n) => n.id));
    const safe = (id) => "n_" + id.replace(/[^a-zA-Z0-9_]/g, "_");
    visible.forEach((n) => {
      const label = (n.fqn || n.name || n.id).replace(/"/g, "'");
      lines.push("  " + safe(n.id) + '["' + label + '"]');
    });
    (DATA.edges || []).forEach((e) => {
      if (ids.has(e.from) && ids.has(e.to)) {
        lines.push("  " + safe(e.from) + " --> " + safe(e.to));
      }
    });
    downloadBlob(lines.join("\n") + "\n", "simplineage-lineage.mmd", "text/plain");
  }

  function downloadBlob(text, filename, type) {
    const blob = new Blob([text], { type: type });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }
  function escapeAttr(s) {
    return escapeHtml(s).replace(/'/g, "&#39;");
  }

  // boot
  loadTheme();
  initChrome();
  draw();
  fitView();
})();
