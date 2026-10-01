/* Optional development integration test. No browser dependencies ship with hyp. */
const { chromium } = require("playwright");
const { spawn, execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const assert = require("node:assert/strict");
const bin =
  process.env.HYP_BIN || path.resolve(__dirname, "../target/debug/hyp");
const root = fs.mkdtempSync(path.join(os.tmpdir(), "hyp-browser-"));
const cli = (...args) =>
  execFileSync(bin, ["--project", root, ...args], { encoding: "utf8" }).trim();
let server, browser;
const errors = [];
async function main() {
  cli("init", "--demo");
  server = spawn(bin, ["--project", root, "web", "--port", "0"]);
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("server startup timed out")),
      10000,
    );
    server.stdout.on("data", (data) => {
      const match = String(data).match(/http:\/\/127\.0\.0\.1:\d+/);
      if (match) {
        clearTimeout(timer);
        resolve(match[0]);
      }
    });
    server.on("exit", (code) => reject(new Error("server exited " + code)));
  });
  browser = await chromium.launch({
    headless: true,
    ...(process.env.CHROMIUM_PATH
      ? { executablePath: process.env.CHROMIUM_PATH }
      : {}),
  });
  const page = await browser.newPage({
    viewport: { width: 1440, height: 1050 },
  });
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (msg) => {
    if (
      msg.type() === "error" &&
      !msg.text().includes("favicon") &&
      !msg.text().includes("409")
    )
      errors.push(msg.text());
  });
  await page.goto(url);
  await page
    .getByRole("heading", { name: "Hypotheses", exact: true })
    .waitFor();
  await page
    .getByRole("heading", {
      name: "DMA timeout is caused by cache coherency",
      exact: true,
    })
    .waitFor();
  if (process.env.HYP_SCREENSHOT_DIR) {
    fs.mkdirSync(process.env.HYP_SCREENSHOT_DIR, { recursive: true });
    await page.screenshot({
      path: path.join(process.env.HYP_SCREENSHOT_DIR, "overview.png"),
      fullPage: true,
    });
  }
  await page
    .getByRole("button", { name: "＋ New hypothesis", exact: true })
    .click();
  await page.getByLabel("Statement / title").fill("Browser-created hypothesis");
  await page.getByLabel("Scope", { exact: true }).fill("A browser test");
  await page
    .getByLabel("Notes", { exact: true })
    .fill("A draft written through the UI");
  await page.getByRole("button", { name: "Save record", exact: true }).click();
  await page
    .getByRole("heading", { name: "Browser-created hypothesis", exact: true })
    .waitFor();
  const created = JSON.parse(
    cli("--json", "list", "--kind", "hypothesis"),
  ).find((e) => e.record.title === "Browser-created hypothesis");
  assert.ok(created, "GUI write visible in CLI");
  cli("set", created.record.id, "--title", "Changed from CLI");
  await page
    .getByRole("heading", { name: "Changed from CLI", exact: true })
    .waitFor();
  // A second tab sees file changes as well.
  const second = await browser.newPage();
  await second.goto(url + "/#record/" + created.record.id);
  await second
    .getByRole("heading", { name: "Changed from CLI", exact: true })
    .waitFor();
  const filename = path.join(
    root,
    "hyp",
    "hypotheses",
    created.record.id + ".md",
  );
  fs.writeFileSync(
    filename,
    fs
      .readFileSync(filename, "utf8")
      .replace("Changed from CLI", "Changed by editor"),
  );
  await page
    .getByRole("heading", { name: "Changed by editor", exact: true })
    .waitFor();
  await second
    .getByRole("heading", { name: "Changed by editor", exact: true })
    .waitFor();
  // Dirty form is preserved, stale save rejected.
  await page.getByRole("button", { name: "Edit record", exact: true }).click();
  await page.getByLabel("Statement / title").fill("My unsaved draft");
  cli("set", created.record.id, "--title", "Concurrent change");
  await page
    .locator("#notice")
    .filter({ hasText: "while you were editing" })
    .waitFor();
  assert.equal(
    await page.getByLabel("Statement / title").inputValue(),
    "My unsaved draft",
  );
  await page.getByRole("button", { name: "Save record", exact: true }).click();
  await page.locator("#form-error").filter({ hasText: "conflict:" }).waitFor();
  assert.equal(
    JSON.parse(cli("--json", "show", created.record.id)).entry.record.title,
    "Concurrent change",
  );
  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  // Browser creation of a criterion and explicit assessment.
  await page
    .locator("section")
    .filter({
      has: page.getByRole("heading", { name: /What would falsify this/ }),
    })
    .getByRole("button", { name: "＋ Add", exact: true })
    .click();
  await page
    .getByLabel("Statement / title")
    .fill("Reject if the observed output is zero");
  await page.getByRole("button", { name: "Save record", exact: true }).click();
  await page
    .getByRole("heading", {
      name: "Reject if the observed output is zero",
      exact: true,
    })
    .waitFor();
  const criterion = JSON.parse(
    cli("--json", "list", "--kind", "criterion"),
  ).find((e) => e.record.title === "Reject if the observed output is zero");
  assert.equal(criterion.record.hypothesis, created.record.id);
  // Any judgment but untested cites evidence linked to the hypothesis
  // (decision-0003): link some first, then cite it in the form.
  const evidence = JSON.parse(
    cli(
      "--json",
      "evidence",
      "add",
      created.record.id,
      "First measurement is ambiguous",
      "--source",
      "bench.log",
      "--qualifies",
    ),
  ).written.find((w) => w.kind === "evidence").id;
  await page.goto(url + "/#record/" + created.record.id);
  // The form offers what the page has seen: wait for the live update.
  await page.getByText("First measurement is ambiguous").first().waitFor();
  await page
    .getByRole("button", { name: "Review hypothesis", exact: true })
    .click();
  await page
    .getByLabel("Statement / title")
    .fill("Inconclusive after first review");
  await page.getByLabel("Evidence considered").selectOption(evidence);
  await page
    .getByLabel("Assessment rationale")
    .fill("No measurement has been recorded yet.");
  await page.getByRole("button", { name: "Save record", exact: true }).click();
  await page
    .getByRole("heading", {
      name: "Inconclusive after first review",
      exact: true,
    })
    .waitFor();
  assert.equal(
    JSON.parse(cli("--json", "show", created.record.id)).state.judgment,
    "inconclusive",
  );
  // Matrix, graph and experiment queue are usable.
  for (const [hash, title] of [
    ["matrix", "Evidence matrix"],
    ["graph", "Relationships"],
    ["experiments", "Experiments"],
  ]) {
    await page.goto(url + "/#" + hash);
    await page.getByRole("heading", { name: title, exact: true }).waitFor();
  }
  if (process.env.HYP_SCREENSHOT_DIR) {
    await page.goto(url + "/#matrix");
    await page
      .getByRole("heading", { name: "Evidence matrix", exact: true })
      .waitFor();
    await page.screenshot({
      path: path.join(process.env.HYP_SCREENSHOT_DIR, "matrix.png"),
      fullPage: true,
    });
  }
  // Broken files do not silently disappear or permit writes; repair resumes live state.
  const original = fs.readFileSync(filename, "utf8");
  fs.writeFileSync(filename, "broken");
  await page
    .locator("#notice")
    .filter({ hasText: "Invalid project files" })
    .waitFor();
  fs.writeFileSync(filename, original);
  await page.waitForFunction(() => document.querySelector("#notice").hidden);
  // Static export is genuinely offline and read-only.
  const report = path.join(root, "report.html");
  cli("export", "--format", "html", "--output", report);
  const offline = await browser.newPage();
  offline.on("pageerror", (e) => errors.push(e.message));
  await offline.goto("file://" + report);
  await offline
    .getByRole("heading", { name: "Hypotheses", exact: true })
    .waitFor();
  assert.equal(
    await offline
      .getByRole("button", { name: "＋ New hypothesis", exact: true })
      .isVisible(),
    false,
  );
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(url + "/#overview");
  await page
    .getByRole("heading", { name: "Hypotheses", exact: true })
    .waitFor();
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 1,
    ),
    "mobile page does not overflow",
  );
  if (process.env.HYP_SCREENSHOT_DIR)
    await page.screenshot({
      path: path.join(process.env.HYP_SCREENSHOT_DIR, "mobile.png"),
      fullPage: true,
    });
  assert.deepEqual(errors, []);
  console.log(
    "PASS: GUI CRUD, CLI↔GUI synchronization, editor saves, multi-tab refresh, dirty-form conflict, criteria, assessments, matrix, graph, invalid-file recovery, offline export and mobile layout.",
  );
}
main()
  .catch((e) => {
    console.error(e);
    process.exitCode = 1;
  })
  .finally(async () => {
    if (browser) await browser.close();
    if (server) {
      server.kill("SIGTERM");
      await new Promise((resolve) => server.once("exit", resolve));
    }
    fs.rmSync(root, { recursive: true, force: true });
  });
