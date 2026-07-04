import init, { render_svg_report_from_dsl } from "./pkg/billiards.js";

const sampleDsl = `table brunswick_gc4_9ft
ball cue at center
ball one at (2.18, 4.12)
cue_strike(default).mass_ratio(1.0).energy_loss(0.1)
shot(cue).heading(0deg).speed(medium-soft).tip(side: -0.35R, height: -0.35R).using(default)
trace(max_events: 8)
`;

const input = document.querySelector("#dsl-input");
const status = document.querySelector("#status");
const preview = document.querySelector("#preview");
const previewCard = document.querySelector(".wasm-report-card");
const renderButton = document.querySelector("#render-button");
const resetButton = document.querySelector("#reset-button");
const downloadButton = document.querySelector("#download-button");

let lastSvg = "";

function setStatus(message, kind = "") {
  status.textContent = message;
  status.classList.toggle("ok", kind === "ok");
  status.classList.toggle("error", kind === "error");
}

function setPreviewMessage(message) {
  lastSvg = "";
  preview.replaceChildren();
  const element = document.createElement("p");
  element.className = "empty-preview";
  element.textContent = message;
  preview.appendChild(element);
  if (previewCard) {
    previewCard.dataset.scenarioEvents = "none";
    previewCard.dataset.scenarioEventCount = "0";
    previewCard.dataset.scenarioPlayback = "no-playback";
  }
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}


function eventLogHtml(events) {
  if (events.length === 0) {
    return `<details class="event-log"><summary>Event log</summary><p class="empty-preview">No logged events.</p></details>`;
  }
  const rows = events.map((event) => {
    const label = event.label ?? "";
    const time = Number(event.time ?? 0).toFixed(6);
    const summary = event.summary ?? "";
    const title = event.title ?? `t=${time} ${summary}`;
    return `<li data-event-label="${escapeHtml(label)}" data-event-title="${escapeHtml(title)}" data-event-time="${time}">
      <span class="event-badge">${escapeHtml(label)}</span>
      <span class="event-time">t=${time}</span>
      <span class="event-summary">${escapeHtml(summary)}</span>
    </li>`;
  }).join("");
  return `<details class="event-log" open><summary>Event log</summary><ol class="event-list">${rows}</ol></details>`;
}

function eventBucket(events) {
  if (!Array.isArray(events) || events.length === 0) return "none";
  if (events.length === 1) return "single";
  return "multi";
}

function reportHtml(report, elapsedMs) {
  const events = Array.isArray(report.events)
    ? report.events.map((event) => Array.isArray(event)
      ? { label: String(event[0] ?? ""), time: Number(event[1]), summary: String(event[2] ?? ""), title: String(event[3] ?? "") }
      : event)
    : [];
  const playback = report.playback ?? null;
  const svgSizeKiB = (new Blob([report.svg], { type: "image/svg+xml" }).size / 1024).toFixed(1);
  const duration = playback ? `${Number(playback.duration || 0).toFixed(3)} s` : "static layout";
  const frameCount = playback?.frames?.length ?? 0;
  const eventCount = events.length;
  const viewerControlsHtml = window.BilliardsReportViewer.viewerControlsHtml({ tableDetailDefault: "full" });
  const playbackPanelHtml = window.BilliardsReportViewer.playbackPanelHtml(playback);
  if (previewCard) {
    previewCard.dataset.scenarioSearch = `${input.value} ${events.map((event) => event.summary).join(" ")}`.toLowerCase();
    previewCard.dataset.scenarioEvents = eventBucket(events);
    previewCard.dataset.scenarioEventCount = String(eventCount);
    previewCard.dataset.scenarioPlayback = playback ? "with-playback" : "no-playback";
  }
  return `
    <div class="card-workspace">
      <div class="card-overview card-overview-full">
        <div class="info-panel">
          <dl class="info-table">
            <div class="info-row"><dt>Renderer</dt><dd>Rust/Wasm SVG generator</dd></div>
            <div class="info-row"><dt>SVG size</dt><dd>${svgSizeKiB} KiB</dd></div>
            <div class="info-row"><dt>Render time</dt><dd>${elapsedMs} ms</dd></div>
            <div class="info-row"><dt>Duration</dt><dd>${duration}</dd></div>
            <div class="info-row"><dt>Frames</dt><dd>${frameCount}</dd></div>
            <div class="info-row"><dt>Events</dt><dd>${eventCount}</dd></div>
          </dl>
        </div>
      </div>
      <figure class="svg-viewer" data-viewer>
        ${viewerControlsHtml}
        <div class="svg-frame">${report.svg}</div>
        ${playbackPanelHtml}
        <div class="downloads">Download: <button id="inline-download-button" type="button">SVG</button></div>
      </figure>
      ${eventLogHtml(events)}
    </div>`;
}

function render() {
  try {
    const startedAt = performance.now();
    const report = JSON.parse(render_svg_report_from_dsl(input.value));
    lastSvg = report.svg;
    const elapsedMs = Math.round(performance.now() - startedAt);
    preview.innerHTML = reportHtml(report, elapsedMs);
    preview.querySelector("#inline-download-button")?.addEventListener("click", downloadSvg);
    window.BilliardsReportViewer?.initialize(preview);
    downloadButton.disabled = false;
    const sizeKiB = (new Blob([lastSvg], { type: "image/svg+xml" }).size / 1024).toFixed(1);
    const frames = report.playback?.frames?.length ?? 0;
    setStatus(`Rendered ${sizeKiB} KiB SVG in ${elapsedMs} ms${frames ? ` with ${frames} playback frames` : ""}.`, "ok");
  } catch (error) {
    downloadButton.disabled = true;
    setPreviewMessage("The renderer rejected this scenario. See the error below the editor.");
    setStatus(error instanceof Error ? error.message : String(error), "error");
  }
}

function downloadSvg() {
  if (!lastSvg) return;
  const blob = new Blob([lastSvg], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "billiards-scenario.svg";
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

async function boot() {
  input.value = sampleDsl;
  renderButton.disabled = true;
  resetButton.disabled = true;
  downloadButton.disabled = true;

  try {
    await init();
    renderButton.disabled = false;
    resetButton.disabled = false;
    renderButton.addEventListener("click", render);
    resetButton.addEventListener("click", () => {
      input.value = sampleDsl;
      render();
    });
    downloadButton.addEventListener("click", downloadSvg);
    render();
  } catch (error) {
    setPreviewMessage("Wasm package not loaded. Run `just wasm-web`, then serve the `web/` directory over HTTP.");
    setStatus(error instanceof Error ? error.message : String(error), "error");
  }
}

boot();
