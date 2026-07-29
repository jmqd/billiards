#!/usr/bin/env node

import net from "node:net";
import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const CHROME_ARGUMENTS = [
  "--headless=new",
  "--no-first-run",
  "--disable-background-timer-throttling",
  "--disable-renderer-backgrounding",
];
const EXPECTED_EVALUATION_COUNT = "256 actual · 256 planned · 256 maximum";
const DEFAULT_PLAYER_LEVEL = "a";
const WORLD_CLASS_PLAYER_LEVEL = "world-class-pro";
const WORLD_CLASS_PLAYER_LABEL = "World Class Pro";
const EXPECTED_WORLD_CLASS_NOISE_SUMMARY = "World Class Pro: σ 0.175° heading · 1.05 ips speed · 0.0056 R side · 0.0056 R height · 0.105° elevation";
const DRIVER_START_TIMEOUT_MS = 30_000;
const COMMAND_TIMEOUT_MS = 30_000;
const PROBE_TIMEOUT_MS = 30_000;
const PAGE_READY_TIMEOUT_MS = 60_000;
const ROBUST_SEARCH_TIMEOUT_MS = 120_000;
const UNAVAILABLE_SCREENSHOT_PNG_BASE64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

const delay = (milliseconds) => new Promise((resolve) => {
  setTimeout(resolve, milliseconds);
});

function normalizeError(error) {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack ?? null,
      webdriver: error.webdriver ?? null,
    };
  }
  return {
    name: "Error",
    message: String(error),
    stack: null,
    webdriver: null,
  };
}

function formatState(state) {
  try {
    return JSON.stringify(state);
  } catch {
    return String(state);
  }
}

function pageUrl(baseUrl, pathname) {
  return new URL(pathname.replace(/^\//, ""), baseUrl).href;
}

async function reserveLoopbackPort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    server.close();
    throw new Error("Failed to reserve a loopback port for ChromeDriver.");
  }
  await new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
  return address.port;
}

function launchChromeDriver(binary, port, logPath) {
  const log = createWriteStream(logPath, { flags: "a" });
  let logError = null;
  log.on("error", (error) => {
    logError = error;
  });

  const child = spawn(binary, [`--port=${port}`, "--verbose"], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let spawnError = null;
  let closeResult = null;
  child.once("error", (error) => {
    spawnError = error;
    if (!log.destroyed) log.write(`ChromeDriver spawn error: ${error.stack ?? error.message}\n`);
  });
  child.stdout.on("data", (chunk) => {
    if (!log.destroyed) log.write(chunk);
  });
  child.stderr.on("data", (chunk) => {
    if (!log.destroyed) log.write(chunk);
  });
  const closed = new Promise((resolve) => {
    child.once("close", (code, signal) => {
      closeResult = { code, signal };
      resolve(closeResult);
    });
  });

  return {
    child,
    closed,
    log,
    get closeResult() {
      return closeResult;
    },
    get logError() {
      return logError;
    },
    get spawnError() {
      return spawnError;
    },
  };
}

async function waitForChromeDriver(driver, endpoint) {
  const deadline = Date.now() + DRIVER_START_TIMEOUT_MS;
  let lastError = null;

  while (Date.now() < deadline) {
    if (driver.spawnError) throw driver.spawnError;
    if (driver.closeResult) {
      throw new Error(
        `ChromeDriver exited before becoming ready: ${formatState(driver.closeResult)}`,
      );
    }

    try {
      const response = await fetch(new URL("status", endpoint), {
        signal: AbortSignal.timeout(1_000),
      });
      if (response.ok) {
        const payload = await response.json();
        if (payload?.value?.ready !== false) return payload.value;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }

  const detail = lastError instanceof Error ? ` Last error: ${lastError.message}` : "";
  throw new Error(`ChromeDriver did not become ready within 30 seconds.${detail}`);
}

async function terminateChromeDriver(driver) {
  if (!driver) return;
  const { child } = driver;
  if (child.exitCode === null && child.signalCode === null && !driver.closeResult) {
    child.kill("SIGTERM");
    await Promise.race([driver.closed, delay(5_000)]);
  }
  if (child.exitCode === null && child.signalCode === null && !driver.closeResult) {
    child.kill("SIGKILL");
    await driver.closed;
  }
  if (!driver.log.destroyed) {
    await new Promise((resolve) => driver.log.end(resolve));
  }
}

class WebDriverClient {
  constructor(endpoint) {
    this.endpoint = endpoint;
    this.sessionId = null;
  }

  async request(method, pathname, body, timeoutMs = COMMAND_TIMEOUT_MS) {
    const options = {
      method,
      signal: AbortSignal.timeout(timeoutMs),
    };
    if (body !== undefined) {
      options.headers = { "content-type": "application/json" };
      options.body = JSON.stringify(body);
    }

    let response;
    try {
      response = await fetch(new URL(pathname.replace(/^\//, ""), this.endpoint), options);
    } catch (error) {
      throw new Error(
        `WebDriver ${method} ${pathname} request failed: ${error.message}`,
        { cause: error },
      );
    }

    const responseText = await response.text();
    let payload = null;
    if (responseText) {
      try {
        payload = JSON.parse(responseText);
      } catch (error) {
        throw new Error(
          `WebDriver ${method} ${pathname} returned invalid JSON: ${responseText}`,
          { cause: error },
        );
      }
    }

    if (!response.ok) {
      const detail = payload?.value?.message ?? responseText ?? response.statusText;
      const error = new Error(
        `WebDriver ${method} ${pathname} failed with HTTP ${response.status}: ${detail}`,
      );
      error.webdriver = payload?.value ?? payload;
      throw error;
    }
    return payload?.value ?? null;
  }

  async createSession(chromeBinary) {
    const chromeOptions = { args: CHROME_ARGUMENTS };
    if (chromeBinary) chromeOptions.binary = chromeBinary;
    const value = await this.request("POST", "/session", {
      capabilities: {
        alwaysMatch: {
          browserName: "chrome",
          "goog:loggingPrefs": { browser: "ALL" },
          "goog:chromeOptions": chromeOptions,
        },
      },
    }, 60_000);
    if (!value || typeof value.sessionId !== "string" || !value.sessionId) {
      throw new Error(`ChromeDriver returned an invalid session response: ${formatState(value)}`);
    }
    this.sessionId = value.sessionId;
    return value.capabilities ?? {};
  }

  sessionPath(suffix = "") {
    if (!this.sessionId) throw new Error("WebDriver session has not been created.");
    return `/session/${encodeURIComponent(this.sessionId)}${suffix}`;
  }

  navigate(url) {
    return this.request("POST", this.sessionPath("/url"), { url }, 60_000);
  }

  execute(script, args = []) {
    return this.request("POST", this.sessionPath("/execute/sync"), { script, args });
  }

  browserLogs() {
    return this.request("POST", this.sessionPath("/log"), { type: "browser" });
  }

  screenshot() {
    return this.request("GET", this.sessionPath("/screenshot"));
  }

  async deleteSession() {
    if (!this.sessionId) return;
    const sessionId = this.sessionId;
    this.sessionId = null;
    await this.request("DELETE", `/session/${encodeURIComponent(sessionId)}`);
  }
}

async function waitForState({ description, timeoutMs, read, ready, failed }) {
  const deadline = Date.now() + timeoutMs;
  let state = null;

  while (Date.now() < deadline) {
    state = await read();
    if (failed?.(state)) {
      throw new Error(`${description} failed: ${formatState(state)}`);
    }
    if (ready(state)) return state;
    await delay(100);
  }

  throw new Error(
    `${description} timed out after ${Math.round(timeoutMs / 1_000)} seconds. Last state: ${formatState(state)}`,
  );
}

async function runHtmlProbe(client, baseUrl, pathname) {
  const url = pageUrl(baseUrl, pathname);
  await client.navigate(url);
  const state = await waitForState({
    description: `${pathname} probe`,
    timeoutMs: PROBE_TIMEOUT_MS,
    read: () => client.execute(`
      return {
        status: document.documentElement.dataset.testStatus || null,
        result: document.querySelector('#result')?.textContent ?? null,
      };
    `),
    ready: (value) => value?.status === "passed",
    failed: (value) => value?.status === "failed",
  });
  return { pathname, status: state.status, result: state.result };
}

async function waitForProductionPage(client, baseUrl) {
  await client.navigate(pageUrl(baseUrl, "/index.html"));
  return waitForState({
    description: "production Wasm page initialization",
    timeoutMs: PAGE_READY_TIMEOUT_MS,
    read: () => client.execute(`
      const status = document.querySelector('#status');
      const searchButton = document.querySelector('#robust-search-button');
      return {
        status: status?.textContent?.trim() ?? null,
        statusIsError: status?.classList.contains('error') ?? false,
        searchButtonDisabled: searchButton?.disabled ?? null,
        robustStatus: document.querySelector('#robust-search-status')?.textContent?.trim() ?? null,
      };
    `),
    ready: (value) => value?.status?.startsWith("Rendered ")
      && value.searchButtonDisabled === false,
    failed: (value) => value?.statusIsError === true,
  });
}

async function waitForSearchButton(client, expectedPlayerLevel) {
  const state = await waitForState({
    description: "robust search form recovery",
    timeoutMs: 5_000,
    read: () => client.execute(`
      const button = document.querySelector('#robust-search-button');
      const select = document.querySelector('#robust-player-level');
      return {
        present: Boolean(button),
        disabled: button?.disabled ?? null,
        label: button?.textContent?.trim() ?? null,
        iterations: document.querySelector('#robust-iterations')?.value ?? null,
        playerLevel: select?.value ?? null,
        playerLevels: Array.from(select?.options ?? [], (option) => ({
          value: option.value,
          label: option.textContent?.trim() ?? null,
        })),
      };
    `),
    ready: (value) => value?.present === true && value.disabled === false,
  });
  const worldClassOption = state.playerLevels.find(
    (option) => option.value === WORLD_CLASS_PLAYER_LEVEL,
  );
  if (
    state.iterations !== "256"
    || state.playerLevel !== expectedPlayerLevel
    || worldClassOption?.label !== WORLD_CLASS_PLAYER_LABEL
  ) {
    throw new Error(`Production search options changed: ${formatState(state)}`);
  }
  return state;
}

async function runRobustSearch(
  client,
  {
    expectedInitialPlayerLevel,
    playerLevel,
    playerLevelLabel,
    expectedNoiseSummary,
  },
) {
  await waitForSearchButton(client, expectedInitialPlayerLevel);
  const click = await client.execute(`
    const button = document.querySelector('#robust-search-button');
    const select = document.querySelector('#robust-player-level');
    const iterations = document.querySelector('#robust-iterations')?.value ?? null;
    const targetPlayerLevel = ${JSON.stringify(playerLevel)};
    const option = Array.from(select?.options ?? [])
      .find((candidate) => candidate.value === targetPlayerLevel);
    if (!button || button.disabled || !select || !option || iterations !== '256') {
      return {
        clicked: false,
        present: Boolean(button),
        disabled: button?.disabled ?? null,
        iterations,
        playerLevel: select?.value ?? null,
        playerLevelLabel: option?.textContent?.trim() ?? null,
      };
    }
    select.value = targetPlayerLevel;
    select.dispatchEvent(new Event('change', { bubbles: true }));
    button.click();
    return {
      clicked: true,
      iterations,
      playerLevel: select.value,
      playerLevelLabel: option.textContent?.trim() ?? null,
    };
  `);
  if (
    click?.clicked !== true
    || click.playerLevel !== playerLevel
    || click.playerLevelLabel !== playerLevelLabel
  ) {
    throw new Error(`Could not submit the ${playerLevelLabel} robust search: ${formatState(click)}`);
  }

  const state = await waitForState({
    description: `${playerLevelLabel} robust shot search`,
    timeoutMs: ROBUST_SEARCH_TIMEOUT_MS,
    read: () => client.execute(`
      const status = document.querySelector('#robust-search-status');
      return {
        kind: status?.dataset.kind ?? null,
        status: status?.textContent?.trim() ?? null,
        evaluations: document.querySelector('#robust-evaluation-count')?.textContent?.trim() ?? null,
        noiseSummary: document.querySelector('#robust-noise-summary')?.textContent?.trim() ?? null,
        title: document.querySelector('#robust-search-result-title')?.textContent?.trim() ?? null,
      };
    `),
    ready: (value) => value?.kind === "ok" && value.status?.startsWith("Validated "),
    failed: (value) => value?.kind === "error",
  });

  if (state.evaluations !== EXPECTED_EVALUATION_COUNT) {
    throw new Error(
      `Robust search reported unexpected evaluation counts: ${formatState(state)}`,
    );
  }
  if (state.noiseSummary !== expectedNoiseSummary) {
    throw new Error(`Robust search used unexpected execution error: ${formatState(state)}`);
  }
  const winnerMatch = /^Best validated shot · candidate (\d+)$/.exec(state.title ?? "");
  const winnerId = winnerMatch ? Number(winnerMatch[1]) : Number.NaN;
  if (!Number.isSafeInteger(winnerId) || winnerId < 0) {
    throw new Error(`Robust search reported an invalid winner: ${formatState(state)}`);
  }

  return {
    status: state.status,
    evaluations: state.evaluations,
    noiseSummary: state.noiseSummary,
    title: state.title,
    winnerId,
  };
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

async function captureFailure({ client, error, outputDirectory, state }) {
  const captureErrors = {};
  let browserConsole = [];
  let pageHtml = null;
  let screenshotCaptured = false;

  if (client?.sessionId) {
    try {
      browserConsole = await client.browserLogs();
    } catch (captureError) {
      captureErrors.browserConsole = normalizeError(captureError);
    }
    try {
      pageHtml = await client.execute("return document.documentElement.outerHTML;");
    } catch (captureError) {
      captureErrors.pageHtml = normalizeError(captureError);
    }
    try {
      const screenshot = await client.screenshot();
      if (typeof screenshot !== "string" || !screenshot) {
        throw new Error("WebDriver returned an empty screenshot.");
      }
      await writeFile(path.join(outputDirectory, "screenshot.png"), Buffer.from(screenshot, "base64"));
      screenshotCaptured = true;
    } catch (captureError) {
      captureErrors.screenshot = normalizeError(captureError);
    }
  } else {
    captureErrors.browserSession = {
      name: "Error",
      message: "No browser session was available for page, screenshot, or console capture.",
      stack: null,
      webdriver: null,
    };
  }

  if (pageHtml === null) {
    const reason = captureErrors.pageHtml?.message
      ?? captureErrors.browserSession?.message
      ?? "The current page source was unavailable.";
    pageHtml = `<!doctype html><meta charset="utf-8"><title>Page unavailable</title><pre>${escapeHtml(reason)}</pre>`;
  }
  if (!screenshotCaptured) {
    await writeFile(
      path.join(outputDirectory, "screenshot.png"),
      Buffer.from(UNAVAILABLE_SCREENSHOT_PNG_BASE64, "base64"),
    );
  }

  await writeFile(path.join(outputDirectory, "page.html"), pageHtml);
  await writeFile(
    path.join(outputDirectory, "browser-console.json"),
    `${JSON.stringify({ entries: browserConsole, error: captureErrors.browserConsole ?? null }, null, 2)}\n`,
  );
  await writeFile(
    path.join(outputDirectory, "diagnostics.json"),
    `${JSON.stringify({
      error: normalizeError(error),
      step: state.step,
      previewBaseUrl: state.previewBaseUrl,
      browserCapabilities: state.browserCapabilities,
      htmlProbes: state.htmlProbes,
      robustSearches: state.robustSearches,
      runtime: {
        node: process.version,
        platform: process.platform,
        architecture: process.arch,
      },
      captures: {
        screenshotCaptured,
        errors: captureErrors,
      },
    }, null, 2)}\n`,
  );
}

async function main() {
  const positionalArguments = process.argv.slice(2);
  if (positionalArguments.length !== 1) {
    console.error("Usage: node web/wasm-browser-smoke.mjs <preview-base-url>");
    process.exitCode = 2;
    return;
  }

  let previewBaseUrl;
  try {
    const parsed = new URL(positionalArguments[0]);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      throw new Error(`unsupported protocol ${parsed.protocol}`);
    }
    parsed.hash = "";
    parsed.search = "";
    previewBaseUrl = parsed.href.endsWith("/") ? parsed.href : `${parsed.href}/`;
  } catch (error) {
    console.error(`Invalid preview base URL: ${error.message}`);
    process.exitCode = 2;
    return;
  }

  const chromeDriverBinary = process.env.CHROMEDRIVER_BIN || "chromedriver";
  const chromeBinary = process.env.CHROME_BIN || null;
  const outputDirectory = path.resolve(
    process.env.WASM_SMOKE_OUTPUT_DIR || "target/wasm-browser-smoke",
  );
  await mkdir(outputDirectory, { recursive: true });
  const chromeDriverLogPath = path.join(outputDirectory, "chromedriver.log");
  await writeFile(chromeDriverLogPath, "");

  const state = {
    step: "reserve ChromeDriver port",
    previewBaseUrl,
    browserCapabilities: null,
    htmlProbes: [],
    robustSearches: [],
  };
  let driver = null;
  let client = null;
  let failure = null;

  try {
    const port = await reserveLoopbackPort();
    const endpoint = `http://127.0.0.1:${port}/`;

    state.step = "start ChromeDriver";
    driver = launchChromeDriver(chromeDriverBinary, port, chromeDriverLogPath);
    await waitForChromeDriver(driver, endpoint);

    state.step = "create Chrome session";
    client = new WebDriverClient(endpoint);
    state.browserCapabilities = await client.createSession(chromeBinary);

    state.step = "run render worker protocol probe";
    state.htmlProbes.push(await runHtmlProbe(
      client,
      previewBaseUrl,
      "/render-worker.test.html",
    ));

    state.step = "run application protocol probe";
    state.htmlProbes.push(await runHtmlProbe(
      client,
      previewBaseUrl,
      "/app.test.html",
    ));

    state.step = "initialize production Wasm page";
    await waitForProductionPage(client, previewBaseUrl);

    const worldClassProfile = {
      playerLevel: WORLD_CLASS_PLAYER_LEVEL,
      playerLevelLabel: WORLD_CLASS_PLAYER_LABEL,
      expectedNoiseSummary: EXPECTED_WORLD_CLASS_NOISE_SUMMARY,
    };
    state.step = "run first robust shot search";
    const firstSearch = await runRobustSearch(client, {
      expectedInitialPlayerLevel: DEFAULT_PLAYER_LEVEL,
      ...worldClassProfile,
    });
    state.robustSearches.push(firstSearch);

    state.step = "run second robust shot search";
    const secondSearch = await runRobustSearch(client, {
      expectedInitialPlayerLevel: WORLD_CLASS_PLAYER_LEVEL,
      ...worldClassProfile,
    });
    state.robustSearches.push(secondSearch);
    if (secondSearch.winnerId !== firstSearch.winnerId) {
      throw new Error(
        `Robust search winner changed across identical requests: ${firstSearch.winnerId} then ${secondSearch.winnerId}.`,
      );
    }
  } catch (error) {
    failure = error;
    try {
      await captureFailure({ client, error, outputDirectory, state });
    } catch (captureError) {
      console.error("Failed to write complete browser diagnostics", captureError);
    }
  } finally {
    if (client?.sessionId) {
      try {
        await client.deleteSession();
      } catch (error) {
        console.error("Failed to delete the WebDriver session", error);
      }
    }
    await terminateChromeDriver(driver);
    if (driver?.logError) {
      console.error("ChromeDriver log stream failed", driver.logError);
    }
  }

  if (failure) {
    console.error(`Wasm browser smoke failed during ${state.step}: ${normalizeError(failure).message}`);
    console.error(`Diagnostics: ${outputDirectory}`);
    process.exitCode = 1;
    return;
  }

  console.log(JSON.stringify({
    browserCapabilities: state.browserCapabilities,
    htmlProbes: state.htmlProbes,
    robustSearches: state.robustSearches,
  }, null, 2));
}

await main();
