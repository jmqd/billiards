import init, { render_svg_report_from_dsl, robust_three_cushion_shot_from_dsl } from "./pkg/billiards.js";

const wasmInitialization = Promise.resolve().then(() => init());
let fatalReported = false;

async function serializeWorkerError(error, phase) {
  const workerNavigator = globalThis.navigator;
  const userAgentData = workerNavigator?.userAgentData;
  let architecture = null;
  let bitness = null;

  if (typeof userAgentData?.getHighEntropyValues === "function") {
    try {
      const entropy = await userAgentData.getHighEntropyValues(["architecture", "bitness"]);
      if (typeof entropy?.architecture === "string") architecture = entropy.architecture;
      if (typeof entropy?.bitness === "string") bitness = entropy.bitness;
    } catch {
      // High-entropy user agent data is optional diagnostic metadata.
    }
  }

  const isError = error instanceof Error;
  return {
    phase,
    name: isError && typeof error.name === "string" && error.name ? error.name : "Error",
    message: isError ? String(error.message) : String(error),
    stack: isError && typeof error.stack === "string" ? error.stack : null,
    runtime: {
      userAgent: typeof workerNavigator?.userAgent === "string" ? workerNavigator.userAgent : "",
      platform: typeof userAgentData?.platform === "string"
        ? userAgentData.platform
        : typeof workerNavigator?.platform === "string"
          ? workerNavigator.platform
          : "",
      architecture,
      bitness,
      hardwareConcurrency: typeof workerNavigator?.hardwareConcurrency === "number"
        && Number.isFinite(workerNavigator.hardwareConcurrency)
        ? workerNavigator.hardwareConcurrency
        : null,
    },
  };
}

function parseWasmJson(value) {
  return typeof value === "string" ? JSON.parse(value) : value;
}

self.addEventListener("message", async (event) => {
  const { id, source, action = "render", iterations, playerLevel } = event.data ?? {};
  if (!Number.isSafeInteger(id) || typeof source !== "string") return;
  let phase = "dispatch";

  try {
    await wasmInitialization;
  } catch (error) {
    if (fatalReported) return;
    fatalReported = true;
    const serializedError = await serializeWorkerError(error, "initialization");
    self.postMessage({ fatal: true, error: serializedError });
    self.close();
    return;
  }

  try {
    const startedAt = performance.now();
    if (action === "robust-shot-search") {
      phase = "robust-shot-search";
      const result = robust_three_cushion_shot_from_dsl(source, iterations, playerLevel);
      const search = parseWasmJson(result);
      const elapsedMs = Math.round(performance.now() - startedAt);
      self.postMessage({ id, action, search, elapsedMs });
      return;
    }
    if (action !== "render") {
      throw new Error(`unknown worker action: ${action}`);
    }
    phase = "render";
    const report = parseWasmJson(render_svg_report_from_dsl(source));
    const elapsedMs = Math.round(performance.now() - startedAt);
    self.postMessage({ id, action, report, elapsedMs });
  } catch (error) {
    self.postMessage({ id, action, error: await serializeWorkerError(error, phase) });
  }
});
