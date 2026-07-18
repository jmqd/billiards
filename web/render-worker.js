import init, { render_svg_report_from_dsl } from "./pkg/billiards.js";

const wasmReady = init();

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function parseWasmJson(value) {
  return typeof value === "string" ? JSON.parse(value) : value;
}

self.addEventListener("message", async (event) => {
  const { id, source } = event.data ?? {};
  if (!Number.isSafeInteger(id) || typeof source !== "string") return;

  try {
    await wasmReady;
    const startedAt = performance.now();
    const report = parseWasmJson(render_svg_report_from_dsl(source));
    const elapsedMs = Math.round(performance.now() - startedAt);
    self.postMessage({ id, report, elapsedMs });
  } catch (error) {
    self.postMessage({ id, error: errorMessage(error) });
  }
});
