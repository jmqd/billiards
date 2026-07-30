import init, {
  apply_robust_shot_candidate_to_dsl,
  shot_controls_from_dsl,
  update_shot_control_in_dsl,
  update_shot_tip_in_dsl,
} from "./pkg/billiards.js";

const sampleDsl = `# Legal three-cushion scoring shot: cue contacts yellow, then left/top/right cushions, then red.
# Goal: compact plus-English scoring path across three different cushions.
table three_cushion_carom_10ft
game three_cushion
ball cue at (3.354, 3.309)
ball yellow at (2.491, 5.838)
ball red at (2.762, 3.888)
cue_strike(default).mass_ratio(1.0).energy_loss(0.08)
ball_ball(carom).normal_restitution(0.98).tangential_friction(0.05)
rail_response(lively).normal_restitution(0.82).tangential_friction(0.82)
rails(carom).default(lively)
simulation(default)
  .collision_model(throw_aware)
  .ball_ball(carom)
  .rail_model(spin_aware)
  .rails(carom)
  .conditions(heated_carom)
  .max_events(24)
trace(max_events: 24)
shot(cue).heading(341.141deg).speed(108ips).tip(side: 0.39R, height: 0.11R).using(default)`;

const TEXTAREA_RENDER_DELAY_MS = 250;
const CONTROL_RENDER_DELAY_MS = 75;
const TIP_PAD_VIEW_RADIUS = 1.12;
const IPS_TO_KMH = 0.09144;
const WORKER_ERROR_PHASES = new Set([
  "initialization",
  "dispatch",
  "render",
  "robust-shot-search",
]);

const input = document.querySelector("#dsl-input");
const status = document.querySelector("#status");
const preview = document.querySelector("#preview");
const previewCard = document.querySelector(".wasm-report-card");
const previewState = document.querySelector("#preview-state");
const renderButton = document.querySelector("#render-button");
const resetButton = document.querySelector("#reset-button");
const downloadButton = document.querySelector("#download-button");
const shotControlsPanel = document.querySelector("#shot-controls");
const shotFieldsets = [...shotControlsPanel.querySelectorAll("fieldset")];

const headingDial = document.querySelector("#heading-dial");
const headingNeedle = document.querySelector("#heading-needle");
const headingInput = document.querySelector("#heading-input");
const headingNudges = [...document.querySelectorAll("[data-heading-delta]")];

const tipPad = document.querySelector("#tip-pad");
const tipPadDescription = document.querySelector("#tip-pad-description");
const tipLimitRing = document.querySelector("#tip-limit-ring");
const tipPointer = document.querySelector("#tip-pointer");
const tipSideRange = document.querySelector("#tip-side-range");
const tipSideInput = document.querySelector("#tip-side-input");
const tipHeightRange = document.querySelector("#tip-height-range");
const tipHeightInput = document.querySelector("#tip-height-input");

const speedRange = document.querySelector("#speed-range");
const speedInput = document.querySelector("#speed-input");
const speedHint = document.querySelector("#speed-hint");
const elevationRange = document.querySelector("#elevation-range");
const elevationInput = document.querySelector("#elevation-input");
const elevationMode = document.querySelector("#elevation-mode");

const robustSearchPanel = document.querySelector("#robust-search");
const robustSearchForm = document.querySelector("#robust-search-form");
const robustIterationsInput = document.querySelector("#robust-iterations");
const robustPlayerLevelSelect = document.querySelector("#robust-player-level");
const robustSearchButton = document.querySelector("#robust-search-button");
const robustSearchStatus = document.querySelector("#robust-search-status");
const robustSearchResults = document.querySelector("#robust-search-results");
const robustSearchResultTitle = document.querySelector("#robust-search-result-title");
const robustApplyButton = document.querySelector("#robust-apply-button");
const robustWinnerProbability = document.querySelector("#robust-winner-probability");
const robustWinnerInterval = document.querySelector("#robust-winner-interval");
const robustEvaluationCount = document.querySelector("#robust-evaluation-count");
const robustElapsedTime = document.querySelector("#robust-elapsed-time");
const robustNoiseSummary = document.querySelector("#robust-noise-summary");
const robustWinnerControls = document.querySelector("#robust-winner-controls");
const robustRankedList = document.querySelector("#robust-ranked-list");

let lastSvg = "";
let renderWorker = null;
let renderTimer = null;
let renderSequence = 0;
let inFlightRender = null;
const robustSearchRequests = new Map();
let configuredSource = null;
let configuredSourceValid = false;
let renderedSource = null;
let renderedStatus = "";
let shotState = null;
let syncingControls = false;
let wasmReady = false;
let headingPointerId = null;
let tipPointerId = null;
let robustSearchUiSequence = 0;
let robustSearchRunning = false;
let robustSearchResult = null;

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function formatWorkerError(error) {
  const runtime = error?.runtime;
  const valid = error !== null
    && typeof error === "object"
    && !Array.isArray(error)
    && WORKER_ERROR_PHASES.has(error.phase)
    && typeof error.name === "string"
    && typeof error.message === "string"
    && (error.stack === null || typeof error.stack === "string")
    && runtime !== null
    && typeof runtime === "object"
    && !Array.isArray(runtime)
    && typeof runtime.userAgent === "string"
    && typeof runtime.platform === "string"
    && (runtime.architecture === null || typeof runtime.architecture === "string")
    && (runtime.bitness === null || typeof runtime.bitness === "string")
    && (
      runtime.hardwareConcurrency === null
      || (typeof runtime.hardwareConcurrency === "number"
        && Number.isFinite(runtime.hardwareConcurrency))
    );
  if (!valid) return "The background renderer returned an invalid error response.";
  return `${error.phase} failed (${error.name}): ${error.message}`;
}

function reportWorkerError(error) {
  try {
    console.error("Wasm worker failure", JSON.stringify(error));
  } catch (serializationError) {
    console.error("Wasm worker failure", JSON.stringify({
      serializationError: errorMessage(serializationError),
    }));
  }
}

function parseWasmJson(value) {
  return typeof value === "string" ? JSON.parse(value) : value;
}

function setStatus(message, kind = "") {
  status.textContent = message;
  status.classList.toggle("ok", kind === "ok");
  status.classList.toggle("error", kind === "error");
}

function markPreviewStale(stale) {
  const showStaleState = stale && Boolean(lastSvg);
  const downloadsDisabled = stale || !lastSvg;
  previewCard.classList.toggle("is-stale", showStaleState);
  previewCard.dataset.previewStale = showStaleState ? "true" : "false";
  previewState.hidden = !showStaleState;
  downloadButton.disabled = downloadsDisabled;
  const inlineDownloadButton = preview.querySelector("#inline-download-button");
  if (inlineDownloadButton) inlineDownloadButton.disabled = downloadsDisabled;
}

function setPreviewMessage(message) {
  lastSvg = "";
  preview.replaceChildren();
  const element = document.createElement("p");
  element.className = "empty-preview";
  element.textContent = message;
  preview.appendChild(element);
  previewCard.dataset.scenarioEvents = "none";
  previewCard.dataset.scenarioEventCount = "0";
  previewCard.dataset.scenarioPlayback = "no-playback";
  markPreviewStale(false);
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

function finiteContactNumber(value, fallback = 0) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function normalizeContactBall(source) {
  const ball = Array.isArray(source)
    ? { id: source[0], fill: source[1], label: source[2], style: source[5] }
    : source ?? {};
  const fill = String(ball.fill ?? "");
  return {
    id: String(ball.id ?? "ball"),
    fill: /^#[0-9a-f]{6}$/i.test(fill) ? fill : "#f8f4e8",
    label: ball.label == null ? "" : String(ball.label),
    style: String(ball.style ?? "plain"),
  };
}

function contactBallName(ball) {
  if (ball.label) return `${ball.label}-ball`;
  return `${ball.id.replaceAll("-", " ")} ball`;
}

function contactBallSvg(ball, centerX, centerY, radius, { ghost = false } = {}) {
  const x = Number(centerX.toFixed(3));
  const y = Number(centerY.toFixed(3));
  const r = Number(radius.toFixed(3));
  const shellFill = ball.style === "stripe" ? "#f8f4e8" : ball.fill;
  const stripe = ball.style === "stripe"
    ? `<line class="contact-ball-stripe" x1="${x - r * 0.86}" y1="${y}" x2="${x + r * 0.86}" y2="${y}" stroke="${escapeHtml(ball.fill)}" stroke-width="${r * 0.82}"/>`
    : "";
  const label = ball.label
    ? `<circle class="contact-ball-number-disc" cx="${x}" cy="${y}" r="${r * 0.32}"/><text class="contact-ball-number" x="${x}" y="${y}">${escapeHtml(ball.label)}</text>`
    : "";
  return `<g class="contact-ball${ghost ? " contact-ball-ghost" : ""}">
    <circle class="contact-ball-shell" cx="${x}" cy="${y}" r="${r}" fill="${escapeHtml(shellFill)}"/>
    ${stripe}
    ${label}
  </g>`;
}

function firstObjectContactPresentation(contact) {
  if (!contact || typeof contact !== "object") return null;

  const cueBall = normalizeContactBall(contact.cueBall);
  const objectBall = normalizeContactBall(contact.objectBall);
  const cueBallName = contactBallName(cueBall);
  const objectBallName = contactBallName(objectBall);
  const cutAngle = clamp(finiteContactNumber(contact.cutAngleDegrees), 0, 90);
  const hitFraction = clamp(finiteContactNumber(contact.hitFraction), 0, 1);
  const lateralOffset = clamp(finiteContactNumber(contact.lateralOffsetDiameters), -1.05, 1.05);
  const forwardOffset = clamp(finiteContactNumber(contact.forwardOffsetDiameters), -1.05, 1.05);
  const verticalOffset = clamp(finiteContactNumber(contact.verticalOffsetDiameters), -1.05, 1.05);
  const contactTime = Math.max(0, finiteContactNumber(contact.time));
  const airborne = contact.airborne === true;
  const fullnessText = `${Number((hitFraction * 100).toFixed(1))}%`;
  const cutAngleText = `${Number(cutAngle.toFixed(1))}°`;

  const elevatedCue = { x: 72, y: 78 };
  const elevatedObject = {
    x: 72 + lateralOffset * 44,
    y: 72 - forwardOffset * 14 - verticalOffset * 44,
  };
  const topForwardDistance = Math.abs(forwardOffset);
  const topCue = { x: 72, y: 70 };
  const topObject = {
    x: topCue.x + lateralOffset * 40,
    y: topCue.y - topForwardDistance * 40,
  };
  const elevatedLabel = `${fullnessText} full, ${cutAngleText} cut between ${cueBallName} and ${objectBallName}, viewed down the shot line${airborne ? " with airborne contact" : ""}.`;
  const topLabel = `Top-down ghost-ball view of the first contact between ${cueBallName} and ${objectBallName}.`;

  const visualsHtml = `<div class="visual-stack contact-visual-stack" aria-label="First object contact visual aids">
    <div class="visual-panel contact-visual-panel">
      <svg class="contact-elevated-view" viewBox="0 0 144 112" role="img" aria-label="${escapeHtml(elevatedLabel)}">
        <path class="contact-table-plane" d="M8 102 L136 102 L112 35 L32 35 Z"/>
        <path class="contact-shot-guide" d="M72 104 L72 28"/>
        <path class="contact-shot-arrow" d="M72 25 L67 34 L77 34 Z"/>
        <ellipse class="contact-ball-shadow" cx="${elevatedObject.x}" cy="${elevatedObject.y + 20}" rx="19" ry="5"/>
        ${contactBallSvg(objectBall, elevatedObject.x, elevatedObject.y, 22)}
        <ellipse class="contact-ball-shadow" cx="${elevatedCue.x}" cy="${elevatedCue.y + 20}" rx="19" ry="5"/>
        ${contactBallSvg(cueBall, elevatedCue.x, elevatedCue.y, 22)}
        <text class="contact-view-label" x="72" y="108">DOWN SHOT LINE</text>
      </svg>
      <div class="visual-caption"><strong>Fullness</strong><span>${fullnessText} · ${cutAngleText} cut</span></div>
    </div>
    <div class="visual-panel contact-visual-panel">
      <svg class="contact-top-view" viewBox="0 0 144 112" role="img" aria-label="${escapeHtml(topLabel)}">
        <rect class="contact-table-plane" x="4" y="4" width="136" height="104" rx="4"/>
        <path class="contact-shot-guide" d="M${topCue.x} 102 L${topCue.x} ${topCue.y + 22}"/>
        <path class="contact-shot-arrow" d="M${topCue.x} ${topCue.y + 15} L${topCue.x - 5} ${topCue.y + 24} L${topCue.x + 5} ${topCue.y + 24} Z"/>
        <path class="contact-line-of-centers" d="M${topCue.x} ${topCue.y} L${topObject.x} ${topObject.y}"/>
        ${contactBallSvg(cueBall, topCue.x, topCue.y, 20, { ghost: true })}
        ${contactBallSvg(objectBall, topObject.x, topObject.y, 20)}
        <circle class="contact-point" cx="${(topCue.x + topObject.x) / 2}" cy="${(topCue.y + topObject.y) / 2}" r="2.4"/>
        <text class="contact-view-label" x="72" y="108">BIRD'S-EYE VIEW</text>
      </svg>
      <div class="visual-caption"><strong>Ghost ball</strong><span>${airborne ? "projected · airborne" : "at contact"}</span></div>
    </div>
  </div>`;
  const infoRowsHtml = `
    <div class="info-row"><dt>First contact</dt><dd>${escapeHtml(cueBallName)} → ${escapeHtml(objectBallName)} at ${contactTime.toFixed(3)} s</dd></div>
    <div class="info-row"><dt>Hit fullness</dt><dd>${fullnessText}</dd></div>
    <div class="info-row"><dt>Cut angle</dt><dd>${cutAngleText}${airborne ? " · airborne" : ""}</dd></div>`;

  return { visualsHtml, infoRowsHtml };
}

function reportHtml(report, elapsedMs, source) {
  const events = Array.isArray(report.events)
    ? report.events.map((event) => Array.isArray(event)
      ? { label: String(event[0] ?? ""), time: Number(event[1]), summary: String(event[2] ?? ""), title: String(event[3] ?? "") }
      : event)
    : [];
  const playback = report.playback ?? null;
  const firstObjectContact = firstObjectContactPresentation(report.firstObjectContact);
  const svgSizeKiB = (new Blob([report.svg], { type: "image/svg+xml" }).size / 1024).toFixed(1);
  const duration = playback ? `${Number(playback.duration || 0).toFixed(3)} s` : "static layout";
  const frameCount = playback?.frames?.length ?? 0;
  const eventCount = events.length;
  const viewerControlsHtml = window.BilliardsReportViewer.viewerControlsHtml({ tableDetailDefault: "full" });
  const playbackPanelHtml = window.BilliardsReportViewer.playbackPanelHtml(playback);
  previewCard.dataset.scenarioSearch = `${source} ${events.map((event) => event.summary).join(" ")}`.toLowerCase();
  previewCard.dataset.scenarioEvents = eventBucket(events);
  previewCard.dataset.scenarioEventCount = String(eventCount);
  previewCard.dataset.scenarioPlayback = playback ? "with-playback" : "no-playback";
  return `
    <div class="card-workspace">
      <div class="card-overview${firstObjectContact ? "" : " card-overview-full"}">
        ${firstObjectContact?.visualsHtml ?? ""}
        <div class="info-panel">
          <dl class="info-table">
            <div class="info-row"><dt>Renderer</dt><dd>Rust/Wasm SVG generator</dd></div>
            <div class="info-row"><dt>SVG size</dt><dd>${svgSizeKiB} KiB</dd></div>
            <div class="info-row"><dt>Render time</dt><dd>${elapsedMs} ms</dd></div>
            <div class="info-row"><dt>Duration</dt><dd>${duration}</dd></div>
            <div class="info-row"><dt>Frames</dt><dd>${frameCount}</dd></div>
            <div class="info-row"><dt>Events</dt><dd>${eventCount}</dd></div>
            ${firstObjectContact?.infoRowsHtml ?? ""}
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

function normalizeHeading(value) {
  const wrapped = value % 360;
  if (Object.is(wrapped, -0)) return 0;
  return wrapped < 0 ? wrapped + 360 : wrapped;
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value));
}
function formatControlNumber(value) {
  return String(Number(Number(value).toFixed(6)));
}

function ipsToKmh(value) {
  return Number(value) * IPS_TO_KMH;
}

function kmhToIps(value) {
  return Number(formatControlNumber(Number(value) / IPS_TO_KMH));
}

function setShotControlsAvailable(available) {
  shotControlsPanel.hidden = !available;
  shotFieldsets.forEach((fieldset) => {
    fieldset.disabled = !available;
  });
  if (!available) shotState = null;
}

function syncShotControls(controls) {
  syncingControls = true;
  try {
    shotState = controls;
    setShotControlsAvailable(true);

    const heading = normalizeHeading(Number(controls.headingDegrees));
    const speedKmh = ipsToKmh(controls.speedIps);
    const tipSide = Number(controls.tipSide);
    const tipHeight = Number(controls.tipHeight);
    const tipMaxRadius = Number(controls.tipMaxRadius);
    const elevation = Number(controls.cueElevationDegrees);
    const speedMaxKmh = ipsToKmh(controls.speedMaxIps);
    const elevationMax = Number(controls.cueElevationMaxDegrees);

    const headingText = formatControlNumber(heading);
    headingInput.value = headingText;
    headingDial.setAttribute("aria-valuenow", headingText);
    headingDial.setAttribute("aria-valuetext", `${headingText} degrees`);
    headingNeedle.style.setProperty("--heading-degrees", `${headingText}deg`);

    const tipMinimum = formatControlNumber(-tipMaxRadius);
    const tipMaximum = formatControlNumber(tipMaxRadius);
    const tipSideText = formatControlNumber(tipSide);
    const tipHeightText = formatControlNumber(tipHeight);
    const tipMaxRadiusText = formatControlNumber(tipMaxRadius);
    [tipSideRange, tipSideInput, tipHeightRange, tipHeightInput].forEach((control) => {
      control.min = tipMinimum;
      control.max = tipMaximum;
    });
    tipSideRange.value = tipSideText;
    tipSideInput.value = tipSideText;
    tipHeightRange.value = tipHeightText;
    tipHeightInput.value = tipHeightText;
    tipSideRange.setAttribute("aria-valuetext", `${tipSideText} R`);
    tipHeightRange.setAttribute("aria-valuetext", `${tipHeightText} R`);
    tipLimitRing.setAttribute("r", tipMaxRadiusText);
    tipPointer.setAttribute("cx", tipSideText);
    tipPointer.setAttribute("cy", formatControlNumber(-tipHeight));
    tipPadDescription.textContent = `Current contact is ${tipSideText} R side and ${tipHeightText} R height. Drag within the ${tipMaxRadiusText} R clean-contact limit; side and height controls provide keyboard access.`;

    const speedText = formatControlNumber(speedKmh);
    const speedMaxText = formatControlNumber(speedMaxKmh);
    const speedHintText = String(controls.speedHint);
    speedRange.max = speedMaxText;
    speedInput.max = speedMaxText;
    speedRange.value = speedText;
    speedInput.value = speedText;
    speedHint.value = speedHintText;
    speedHint.textContent = speedHintText;
    speedRange.setAttribute("aria-valuetext", `${speedText} kilometers per hour, ${speedHintText}`);

    const elevationText = formatControlNumber(elevation);
    elevationRange.max = formatControlNumber(elevationMax);
    elevationInput.max = formatControlNumber(elevationMax);
    elevationRange.value = elevationText;
    elevationInput.value = elevationText;
    elevationRange.setAttribute("aria-valuetext", `${elevationText} degrees`);
    elevationMode.value = controls.cueElevationExplicit ? "Explicit" : "Default";
    elevationMode.textContent = elevationMode.value;
  } finally {
    syncingControls = false;
  }
}

function inspectShotControls(source = input.value) {
  try {
    const controls = parseWasmJson(shot_controls_from_dsl(source));
    if (controls === null) {
      setShotControlsAvailable(false);
    } else {
      syncShotControls(controls);
    }
    input.removeAttribute("aria-invalid");
    input.removeAttribute("aria-errormessage");
    return true;
  } catch (error) {
    setShotControlsAvailable(false);
    input.setAttribute("aria-invalid", "true");
    input.setAttribute("aria-errormessage", "status");
    markPreviewStale(true);
    setStatus(errorMessage(error), "error");
    return false;
  }
}

function cancelScheduledRender() {
  if (renderTimer === null) return;
  window.clearTimeout(renderTimer);
  renderTimer = null;
}

function configureSource(source) {
  cancelScheduledRender();
  configuredSource = source;
  configuredSourceValid = false;
  markPreviewStale(source !== renderedSource);
  if (source === renderedSource && renderedStatus) setStatus(renderedStatus, "ok");
  updateRobustSearchFreshness(true);
}

export async function requestRobustThreeCushionSearch({
  source = input.value,
  iterations,
  playerLevel,
} = {}) {
  await billiardsUiReady;
  if (!wasmReady || !renderWorker) {
    return Promise.reject(new Error("The background Wasm worker is not ready."));
  }
  const id = ++renderSequence;
  return new Promise((resolve, reject) => {
    robustSearchRequests.set(id, { resolve, reject });
    try {
      renderWorker.postMessage({
        id,
        action: "robust-shot-search",
        source,
        iterations,
        playerLevel,
      });
    } catch (error) {
      robustSearchRequests.delete(id);
      reject(error);
    }
  });
}

function setRobustSearchStatus(message, kind = "") {
  robustSearchStatus.textContent = message;
  robustSearchStatus.dataset.kind = kind;
}

function updateRobustSearchFreshness(announce = false) {
  let sourceState = "false";
  if (robustSearchResult) {
    if (
      typeof robustSearchResult.appliedSource === "string"
      && robustSearchResult.appliedSource === input.value
    ) {
      sourceState = "applied";
    } else if (robustSearchResult.source !== input.value) {
      sourceState = "changed";
    }
  }

  const actionsDisabled = (
    !wasmReady
    || robustSearchRunning
    || sourceState === "changed"
  );
  const activeCandidateId = sourceState === "applied"
    ? robustSearchResult.appliedCandidateId
    : null;
  const winner = robustSearchResult?.search?.winner;
  const winnerApplied = winner?.candidateId === activeCandidateId;
  robustSearchPanel.dataset.resultStale = sourceState;
  robustApplyButton.disabled = actionsDisabled || !winner || winnerApplied;
  robustApplyButton.dataset.applied = winnerApplied ? "true" : "false";
  robustApplyButton.setAttribute("aria-pressed", winnerApplied ? "true" : "false");
  robustApplyButton.textContent = winnerApplied ? "Best shot applied" : "Apply best shot";

  for (const button of robustRankedList.querySelectorAll(".robust-candidate-apply")) {
    const applied = Number(button.dataset.candidateId) === activeCandidateId;
    button.disabled = actionsDisabled || applied;
    button.dataset.applied = applied ? "true" : "false";
    button.setAttribute("aria-pressed", applied ? "true" : "false");
    button.setAttribute(
      "aria-label",
      applied
        ? `Ranked candidate ${button.dataset.candidateRank} applied`
        : `Apply ranked candidate ${button.dataset.candidateRank}`,
    );
    button.textContent = `${applied ? "Applied" : "Apply"} #${button.dataset.candidateRank}`;
  }

  if (announce && sourceState === "applied") {
    setRobustSearchStatus(
      `Candidate ${robustSearchResult.appliedCandidateId} applied. Choose another validated candidate, or run the search again to evaluate the updated shot.`,
      "ok",
    );
  } else if (announce && sourceState === "changed") {
    setRobustSearchStatus(
      "The DSL changed after this search. Run it again before applying another candidate.",
      "error",
    );
  }
}

function setRobustSearchRunning(running) {
  robustSearchRunning = running;
  robustSearchForm.setAttribute("aria-busy", running ? "true" : "false");
  robustIterationsInput.disabled = running || !wasmReady;
  robustPlayerLevelSelect.disabled = running || !wasmReady;
  robustSearchButton.disabled = running || !wasmReady;
  robustSearchButton.textContent = running ? "Searching…" : "Find robust shot";
  updateRobustSearchFreshness();
}

function formatProbability(value) {
  return Number.isFinite(value) ? `${(value * 100).toFixed(1)}%` : "—";
}

function replaceWinnerControls(controls) {
  robustWinnerControls.replaceChildren();
  if (!controls) return;
  const entries = [
    ["Heading", `${formatControlNumber(controls.headingDegrees)}°`],
    [
      "Speed",
      `${formatControlNumber(ipsToKmh(controls.speedIps))} km/h (${formatControlNumber(controls.speedIps)} ips)`,
    ],
    [
      "Cue tip",
      `${formatControlNumber(controls.tipSide)} R side · ${formatControlNumber(controls.tipHeight)} R height`,
    ],
    ["Cue elevation", `${formatControlNumber(controls.cueElevationDegrees)}°`],
  ];
  for (const [label, value] of entries) {
    const row = document.createElement("div");
    const term = document.createElement("dt");
    const description = document.createElement("dd");
    term.textContent = label;
    description.textContent = value;
    row.append(term, description);
    robustWinnerControls.appendChild(row);
  }
}

function replaceRankedFinalists(finalists) {
  robustRankedList.replaceChildren();
  for (const candidate of finalists ?? []) {
    const summary = candidate.validation ?? candidate.screening;
    const controls = candidate.controls;
    const item = document.createElement("li");
    const description = document.createElement("span");
    const applyButton = document.createElement("button");
    description.className = "robust-ranked-candidate-summary";
    description.textContent = `#${candidate.rank}: ${formatProbability(summary?.successRate)} score probability, ${formatProbability(summary?.confidenceLow)} lower bound — ${formatControlNumber(controls.headingDegrees)}°, ${formatControlNumber(ipsToKmh(controls.speedIps))} km/h, tip (${formatControlNumber(controls.tipSide)}, ${formatControlNumber(controls.tipHeight)}) R, ${formatControlNumber(controls.cueElevationDegrees)}° elevation`;
    applyButton.type = "button";
    applyButton.className = "robust-candidate-apply";
    applyButton.dataset.candidateId = String(candidate.candidateId);
    applyButton.dataset.candidateRank = String(candidate.rank);
    applyButton.dataset.applied = "false";
    applyButton.textContent = `Apply #${candidate.rank}`;
    applyButton.setAttribute("aria-label", `Apply ranked candidate ${candidate.rank}`);
    applyButton.setAttribute("aria-pressed", "false");
    applyButton.addEventListener("click", () => {
      applyRobustSearchCandidate(candidate.candidateId);
    });
    item.append(description, applyButton);
    robustRankedList.appendChild(item);
  }
}

function renderRobustSearchResult(result) {
  const { search, elapsedMs } = result;
  const winner = search.winner;
  const summary = winner?.validation ?? winner?.screening;
  const sigmas = search.shotInaccuracySigmas;
  const applyGuidance = search.sourceHasShot === false
    ? " Applying a candidate will insert a new shot into the DSL."
    : "";

  robustSearchResults.hidden = false;
  robustSearchResultTitle.textContent = winner
    ? `Best validated shot · candidate ${winner.candidateId}`
    : "No eligible validation winner";
  robustWinnerProbability.textContent = formatProbability(summary?.successRate);
  robustWinnerInterval.textContent = summary
    ? `${formatProbability(summary.confidenceLow)}–${formatProbability(summary.confidenceHigh)}`
    : "—";
  robustEvaluationCount.textContent = `${search.actualIterations} actual · ${search.plannedIterations} planned · ${search.requestedIterations} maximum`;
  robustElapsedTime.textContent = `${elapsedMs} ms`;
  robustNoiseSummary.textContent = `${search.playerLevelLabel}: σ ${formatControlNumber(sigmas.headingDegrees)}° heading · ${formatControlNumber(sigmas.speedIps)} ips speed · ${formatControlNumber(sigmas.tipSideRadii)} R side · ${formatControlNumber(sigmas.tipHeightRadii)} R height · ${formatControlNumber(sigmas.cueElevationDegrees)}° elevation`;
  replaceWinnerControls(winner?.controls);
  replaceRankedFinalists(search.rankedFinalists);

  if (winner) {
    setRobustSearchStatus(
      `Validated ${search.validatedFinalistCount} finalists in ${elapsedMs} ms. Candidate ${winner.candidateId} ranked first.${applyGuidance}`,
      "ok",
    );
  } else {
    setRobustSearchStatus(
      `Completed ${search.actualIterations} evaluations, but no finalist produced an eligible validation result.`,
      "error",
    );
  }
  updateRobustSearchFreshness(true);
}

async function runVisibleRobustSearch(event) {
  event.preventDefault();
  if (!robustSearchForm.reportValidity() || robustSearchRunning) return;

  const source = input.value;
  const iterations = robustIterationsInput.valueAsNumber;
  const playerLevel = robustPlayerLevelSelect.value;
  const requestSequence = ++robustSearchUiSequence;
  setRobustSearchRunning(true);
  setRobustSearchStatus(
    `Running up to ${iterations} physics evaluations for the selected player profile…`,
    "running",
  );

  try {
    const result = await requestRobustThreeCushionSearch({
      source,
      iterations,
      playerLevel,
    });
    if (requestSequence !== robustSearchUiSequence) return;
    robustSearchResult = {
      ...result,
      source,
      applied: false,
      appliedCandidateId: null,
      appliedSource: null,
    };
    renderRobustSearchResult(robustSearchResult);
  } catch (error) {
    if (requestSequence !== robustSearchUiSequence) return;
    setRobustSearchStatus(errorMessage(error), "error");
  } finally {
    if (requestSequence === robustSearchUiSequence) setRobustSearchRunning(false);
  }
}

function applyRobustSearchCandidate(candidateId) {
  const result = robustSearchResult;
  const candidate = result?.search?.rankedFinalists?.find(
    (finalist) => finalist.candidateId === candidateId,
  );
  const sourceMatchesSearch = result?.source === input.value;
  const sourceMatchesApplied = result?.appliedSource === input.value;
  if (
    !candidate
    || robustSearchRunning
    || (!sourceMatchesSearch && !sourceMatchesApplied)
  ) {
    return;
  }

  try {
    const controls = candidate.controls;
    const update = parseWasmJson(
      apply_robust_shot_candidate_to_dsl(
        result.source,
        controls.headingDegrees,
        controls.speedIps,
        controls.tipSide,
        controls.tipHeight,
        controls.cueElevationDegrees,
      ),
    );
    applySuccessfulControlUpdate(update);
    result.applied = true;
    result.appliedCandidateId = candidate.candidateId;
    result.appliedSource = update.source;
    updateRobustSearchFreshness(true);
  } catch (error) {
    setRobustSearchStatus(errorMessage(error), "error");
  }
}

function applyRobustSearchWinner() {
  const winner = robustSearchResult?.search?.winner;
  if (winner) applyRobustSearchCandidate(winner.candidateId);
}

function requestConfiguredRender() {
  renderTimer = null;
  if (!wasmReady || !renderWorker || inFlightRender || !configuredSourceValid || configuredSource === renderedSource) return;

  const request = { id: ++renderSequence, source: configuredSource };
  inFlightRender = request;
  renderWorker.postMessage(request);
}

function scheduleConfiguredRender(delay) {
  configuredSourceValid = true;
  cancelScheduledRender();
  if (delay <= 0) {
    requestConfiguredRender();
    return;
  }
  renderTimer = window.setTimeout(requestConfiguredRender, delay);
}

function renderCurrentSource({ inspect = true, delay = 0 } = {}) {
  if (!wasmReady) return;
  const source = input.value;
  configureSource(source);
  if (inspect && !inspectShotControls(source)) return;
  scheduleConfiguredRender(delay);
}

function commitRenderedReport(report, elapsedMs, source) {
  const html = reportHtml(report, elapsedMs, source);

  preview.innerHTML = html;
  preview.querySelector("#inline-download-button")?.addEventListener("click", downloadSvg);
  window.BilliardsReportViewer.initialize(preview);
  lastSvg = report.svg;
  renderedSource = source;
  downloadButton.disabled = false;
  markPreviewStale(false);

  const sizeKiB = (new Blob([lastSvg], { type: "image/svg+xml" }).size / 1024).toFixed(1);
  const frames = report.playback?.frames?.length ?? 0;
  renderedStatus = `Rendered ${sizeKiB} KiB SVG in ${elapsedMs} ms${frames ? ` with ${frames} playback frames` : ""}.`;
  setStatus(renderedStatus, "ok");
}

function failRenderWorker(message) {
  const error = new Error(message);
  for (const request of robustSearchRequests.values()) request.reject(error);
  robustSearchRequests.clear();
  cancelScheduledRender();
  renderWorker?.terminate();
  renderWorker = null;
  inFlightRender = null;
  configuredSourceValid = false;
  wasmReady = false;
  renderButton.disabled = true;
  resetButton.disabled = true;
  setShotControlsAvailable(false);
  markPreviewStale(true);
  setRobustSearchRunning(false);
  setRobustSearchStatus(message, "error");
  setStatus(message, "error");
}

function handleRenderWorkerMessage(event) {
  const response = event.data ?? {};
  if (response.fatal === true) {
    reportWorkerError(response.error);
    failRenderWorker(formatWorkerError(response.error));
    return;
  }

  if (response.action === "robust-shot-search") {
    const request = robustSearchRequests.get(response.id);
    if (!request) return;
    robustSearchRequests.delete(response.id);
    if (Object.prototype.hasOwnProperty.call(response, "error")) {
      reportWorkerError(response.error);
      request.reject(new Error(formatWorkerError(response.error)));
    } else {
      request.resolve({ search: response.search, elapsedMs: response.elapsedMs });
    }
    return;
  }

  const request = inFlightRender;
  if (!request || response.id !== request.id) return;
  inFlightRender = null;
  const failed = Object.prototype.hasOwnProperty.call(response, "error");
  const failureMessage = failed ? formatWorkerError(response.error) : null;
  if (failed) reportWorkerError(response.error);

  if (request.source !== configuredSource) {
    requestConfiguredRender();
    return;
  }

  if (failed) {
    if (!lastSvg) setPreviewMessage("The renderer rejected this scenario. See the exact error below the editor.");
    markPreviewStale(true);
    setStatus(failureMessage, "error");
    return;
  }

  try {
    commitRenderedReport(response.report, response.elapsedMs, request.source);
  } catch (error) {
    if (!lastSvg) setPreviewMessage("The renderer rejected this scenario. See the exact error below the editor.");
    markPreviewStale(true);
    setStatus(errorMessage(error), "error");
    return;
  }

  if (configuredSource !== renderedSource) requestConfiguredRender();
}

function handleRenderWorkerError(event) {
  event.preventDefault();
  failRenderWorker(event.message || "The background renderer stopped unexpectedly. Reload to retry.");
}

function applySuccessfulControlUpdate(update) {
  input.value = update.source;
  syncShotControls(update.controls);
  configureSource(update.source);
  input.removeAttribute("aria-invalid");
  input.removeAttribute("aria-errormessage");
  scheduleConfiguredRender(CONTROL_RENDER_DELAY_MS);
}

function applyControlUpdate(control, value) {
  if (!wasmReady || syncingControls || !shotState || !Number.isFinite(value)) return;
  try {
    const update = parseWasmJson(update_shot_control_in_dsl(input.value, control, value));
    applySuccessfulControlUpdate(update);
  } catch (error) {
    syncShotControls(shotState);
    setStatus(errorMessage(error), "error");
  }
}

function clampTip(side, height) {
  const limit = Number(shotState?.tipMaxRadius ?? 0);
  const radius = Math.hypot(side, height);
  if (radius <= limit || radius === 0) return [side, height];
  const scale = limit / radius;
  return [side * scale, height * scale];
}

function applyTipUpdate(side, height) {
  if (!wasmReady || syncingControls || !shotState || !Number.isFinite(side) || !Number.isFinite(height)) return;
  const [clampedSide, clampedHeight] = clampTip(side, height);
  try {
    const update = parseWasmJson(update_shot_tip_in_dsl(input.value, clampedSide, clampedHeight));
    applySuccessfulControlUpdate(update);
  } catch (error) {
    syncShotControls(shotState);
    setStatus(errorMessage(error), "error");
  }
}

function applyTipAxisUpdate(control, value) {
  if (!shotState || !Number.isFinite(value)) return;
  const isSide = control === "tip-side";
  const otherAxis = Number(isSide ? shotState.tipHeight : shotState.tipSide);
  const limit = Number(shotState.tipMaxRadius);
  const axisLimit = Math.sqrt(Math.max(0, limit * limit - otherAxis * otherAxis));
  applyControlUpdate(control, clamp(value, -axisLimit, axisLimit));
}

function applyHeadingDelta(delta) {
  if (!shotState) return;
  const heading = Number((Number(shotState.headingDegrees) + delta).toFixed(10));
  applyControlUpdate("heading", normalizeHeading(heading));
}

function headingFromPointer(event) {
  const bounds = headingDial.getBoundingClientRect();
  const x = event.clientX - bounds.left - bounds.width / 2;
  const y = event.clientY - bounds.top - bounds.height / 2;
  const degrees = normalizeHeading(Math.atan2(x, -y) * 180 / Math.PI);
  return Math.round(degrees * 10) / 10;
}

function tipFromPointer(event) {
  const bounds = tipPad.getBoundingClientRect();
  const side = ((event.clientX - bounds.left) / bounds.width * 2 - 1) * TIP_PAD_VIEW_RADIUS;
  const height = (1 - (event.clientY - bounds.top) / bounds.height * 2) * TIP_PAD_VIEW_RADIUS;
  return clampTip(Math.round(side * 100) / 100, Math.round(height * 100) / 100);
}

function rangeKeyValue(event, current, step, minimum, maximum) {
  let next = null;
  const arrowStep = event.shiftKey ? step * 10 : step;
  if (event.key === "ArrowUp" || event.key === "ArrowRight") next = current + arrowStep;
  if (event.key === "ArrowDown" || event.key === "ArrowLeft") next = current - arrowStep;
  if (event.key === "PageUp") next = current + step * 10;
  if (event.key === "PageDown") next = current - step * 10;
  if (event.key === "Home") next = minimum;
  if (event.key === "End") next = maximum;
  if (next === null || !Number.isFinite(next)) return null;
  event.preventDefault();
  return clamp(Number(next.toFixed(10)), minimum, maximum);
}

function bindShotControls() {
  headingDial.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    headingPointerId = event.pointerId;
    headingDial.setPointerCapture(event.pointerId);
    applyControlUpdate("heading", headingFromPointer(event));
  });
  headingDial.addEventListener("pointermove", (event) => {
    if (event.pointerId !== headingPointerId) return;
    applyControlUpdate("heading", headingFromPointer(event));
  });
  const endHeadingDrag = (event) => {
    if (event.pointerId === headingPointerId) headingPointerId = null;
  };
  headingDial.addEventListener("pointerup", endHeadingDrag);
  headingDial.addEventListener("pointercancel", endHeadingDrag);
  headingDial.addEventListener("keydown", (event) => {
    let nextHeading = null;
    const arrowStep = event.shiftKey ? 1 : 0.1;
    if (event.key === "ArrowUp" || event.key === "ArrowRight") nextHeading = Number(shotState?.headingDegrees) + arrowStep;
    if (event.key === "ArrowDown" || event.key === "ArrowLeft") nextHeading = Number(shotState?.headingDegrees) - arrowStep;
    if (event.key === "PageUp") nextHeading = Number(shotState?.headingDegrees) + 1;
    if (event.key === "PageDown") nextHeading = Number(shotState?.headingDegrees) - 1;
    if (event.key === "Home") nextHeading = 0;
    if (event.key === "End") nextHeading = 359.9;
    if (nextHeading === null || !Number.isFinite(nextHeading)) return;
    event.preventDefault();
    applyControlUpdate("heading", normalizeHeading(Number(nextHeading.toFixed(10))));
  });
  headingInput.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    const current = headingInput.valueAsNumber;
    if (!Number.isFinite(current)) return;
    event.preventDefault();
    const direction = event.key === "ArrowUp" ? 1 : -1;
    const delta = direction * (event.shiftKey ? 1 : 0.1);
    applyControlUpdate("heading", normalizeHeading(Number((current + delta).toFixed(10))));
  });
  headingInput.addEventListener("change", () => {
    const value = headingInput.valueAsNumber;
    if (Number.isFinite(value)) applyControlUpdate("heading", normalizeHeading(value));
    else if (shotState) syncShotControls(shotState);
  });
  headingNudges.forEach((button) => {
    button.addEventListener("click", () => applyHeadingDelta(Number(button.dataset.headingDelta)));
  });

  tipPad.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    tipPointerId = event.pointerId;
    tipPad.setPointerCapture(event.pointerId);
    applyTipUpdate(...tipFromPointer(event));
  });
  tipPad.addEventListener("pointermove", (event) => {
    if (event.pointerId !== tipPointerId) return;
    applyTipUpdate(...tipFromPointer(event));
  });
  const endTipDrag = (event) => {
    if (event.pointerId === tipPointerId) tipPointerId = null;
  };
  tipPad.addEventListener("pointerup", endTipDrag);
  tipPad.addEventListener("pointercancel", endTipDrag);

  tipSideRange.addEventListener("input", () => applyTipAxisUpdate("tip-side", Number(tipSideRange.value)));
  tipHeightRange.addEventListener("input", () => applyTipAxisUpdate("tip-height", Number(tipHeightRange.value)));
  tipSideRange.addEventListener("keydown", (event) => {
    if (!shotState) return;
    const next = rangeKeyValue(event, Number(shotState.tipSide), 0.01, -Number(shotState.tipMaxRadius), Number(shotState.tipMaxRadius));
    if (next !== null) applyTipAxisUpdate("tip-side", next);
  });
  tipHeightRange.addEventListener("keydown", (event) => {
    if (!shotState) return;
    const next = rangeKeyValue(event, Number(shotState.tipHeight), 0.01, -Number(shotState.tipMaxRadius), Number(shotState.tipMaxRadius));
    if (next !== null) applyTipAxisUpdate("tip-height", next);
  });
  tipSideInput.addEventListener("change", () => {
    const value = tipSideInput.valueAsNumber;
    if (Number.isFinite(value)) applyTipAxisUpdate("tip-side", value);
    else if (shotState) syncShotControls(shotState);
  });
  tipHeightInput.addEventListener("change", () => {
    const value = tipHeightInput.valueAsNumber;
    if (Number.isFinite(value)) applyTipAxisUpdate("tip-height", value);
    else if (shotState) syncShotControls(shotState);
  });

  speedRange.addEventListener("input", () => applyControlUpdate("speed", kmhToIps(Number(speedRange.value))));
  speedRange.addEventListener("keydown", (event) => {
    if (!shotState) return;
    const next = rangeKeyValue(event, ipsToKmh(shotState.speedIps), 0.1, 0, ipsToKmh(shotState.speedMaxIps));
    if (next !== null) applyControlUpdate("speed", kmhToIps(next));
  });
  speedInput.addEventListener("change", () => {
    const value = speedInput.valueAsNumber;
    if (Number.isFinite(value) && shotState) {
      applyControlUpdate("speed", kmhToIps(Math.max(0, value)));
    } else if (shotState) {
      syncShotControls(shotState);
    }
  });

  elevationRange.addEventListener("input", () => applyControlUpdate("elevation", Number(elevationRange.value)));
  elevationRange.addEventListener("keydown", (event) => {
    if (!shotState) return;
    const next = rangeKeyValue(event, Number(shotState.cueElevationDegrees), 0.1, 0, Number(shotState.cueElevationMaxDegrees));
    if (next !== null) applyControlUpdate("elevation", next);
  });
  elevationInput.addEventListener("change", () => {
    const value = elevationInput.valueAsNumber;
    if (Number.isFinite(value) && shotState) {
      applyControlUpdate("elevation", clamp(value, 0, Number(shotState.cueElevationMaxDegrees)));
    } else if (shotState) {
      syncShotControls(shotState);
    }
  });
}

function downloadSvg() {
  if (!lastSvg || previewCard.dataset.previewStale === "true") return;
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
  setShotControlsAvailable(false);
  renderButton.disabled = true;
  resetButton.disabled = true;
  downloadButton.disabled = true;
  setRobustSearchRunning(false);

  try {
    renderWorker = new Worker(new URL("./render-worker.js", import.meta.url), { type: "module" });
    renderWorker.addEventListener("message", handleRenderWorkerMessage);
    renderWorker.addEventListener("error", handleRenderWorkerError);

    await init();
    if (!renderWorker) return;
    wasmReady = true;
    renderButton.disabled = false;
    resetButton.disabled = false;
    setRobustSearchRunning(false);
    setRobustSearchStatus(
      "Choose an evaluation budget and player level, then run the deterministic search.",
    );

    bindShotControls();
    input.addEventListener("input", () => renderCurrentSource({ delay: TEXTAREA_RENDER_DELAY_MS }));
    renderButton.addEventListener("click", () => renderCurrentSource());
    resetButton.addEventListener("click", () => {
      input.value = sampleDsl;
      renderCurrentSource();
    });
    downloadButton.addEventListener("click", downloadSvg);
    robustSearchForm.addEventListener("submit", runVisibleRobustSearch);
    robustApplyButton.addEventListener("click", applyRobustSearchWinner);
    renderCurrentSource();
  } catch (error) {
    renderWorker?.terminate();
    renderWorker = null;
    setShotControlsAvailable(false);
    setPreviewMessage("Wasm package not loaded. Run `just wasm-web`, then serve the `web/` directory over HTTP.");
    setStatus(errorMessage(error), "error");
    setRobustSearchRunning(false);
    setRobustSearchStatus(errorMessage(error), "error");
  }
}

export const billiardsUiReady = boot();
