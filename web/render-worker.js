import init, { render_svg_report_from_dsl } from "./pkg/billiards.js";

const wasmInitialization = Promise.resolve()
  .then(() => init())
  .then(
    () => ({ ok: true }),
    (error) => ({ ok: false, error: errorMessage(error) }),
  );
let fatalReported = false;

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function parseWasmJson(value) {
  return typeof value === "string" ? JSON.parse(value) : value;
}

self.addEventListener("message", async (event) => {
  const { id, source } = event.data ?? {};
  if (!Number.isSafeInteger(id) || typeof source !== "string") return;

  const initialization = await wasmInitialization;
  if (!initialization.ok) {
    if (fatalReported) return;
    fatalReported = true;
    self.postMessage({ fatal: true, error: initialization.error });
    self.close();
    return;
  }

  try {
    const startedAt = performance.now();
    const report = parseWasmJson(render_svg_report_from_dsl(source));
    const elapsedMs = Math.round(performance.now() - startedAt);
    self.postMessage({ id, report, elapsedMs });
  } catch (error) {
    self.postMessage({ id, error: errorMessage(error) });
  }
});
