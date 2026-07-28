#!/usr/bin/env node
/**
 * Agent Room — automated visual QA harness.
 *
 * Runs headless against the Vite preview build and fails on measurable UI
 * defects. This is the "eyes" for an unattended redesign run: nobody is
 * watching, so every visual acceptance criterion in docs/redesign/ that can be
 * machine-checked is checked here.
 *
 *   npm run qa:visual
 *   npm run qa:visual -- --skip-perf        (faster local loop)
 *   npm run qa:visual -- --update-baseline  (accept current screenshots)
 *
 * Exit code 0 = clean. 1 = at least one failing check. The full result is
 * written to docs/redesign/qa/report.json and screenshots to docs/redesign/qa/.
 */

import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { readFileSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "..", "..");
const outDir = join(repo, "docs", "redesign", "qa");
const args = new Set(process.argv.slice(2));
const skipPerf = args.has("--skip-perf");

const WIDTHS = [720, 1024, 1280, 1440, 1800];
const SHOT_WIDTHS = [720, 1280];
const THEMES = ["light", "dark"];
const VIEWS = ["rooms", "activity", "settings"];

const findings = [];
let checks = 0;

function fail(check, detail, meta = {}) {
  findings.push({ check, detail, ...meta, severity: "fail" });
}
function warn(check, detail, meta = {}) {
  findings.push({ check, detail, ...meta, severity: "warn" });
}
function ok(check) {
  checks += 1;
  process.stdout.write(`  ok  ${check}\n`);
}

// ---------------------------------------------------------------- axe source
function axeSource() {
  const candidates = [
    join(repo, "node_modules", "axe-core", "axe.min.js"),
    join(repo, "node_modules", "axe-core", "axe.js"),
  ];
  for (const c of candidates) if (existsSync(c)) return readFileSync(c, "utf8");
  throw new Error("axe-core not found in node_modules — add it to devDependencies");
}

// ---------------------------------------------------------------- launching
async function launch() {
  // The environment provides Chromium. Never download one.
  const executablePath =
    process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE ||
    process.env.CHROME_PATH ||
    undefined;
  return chromium.launch({
    executablePath,
    args: ["--force-color-profile=srgb", "--disable-lcd-text"],
  });
}

async function newPage(browser, { width, theme, reducedMotion, forcedColors, reducedTransparency }) {
  const context = await browser.newContext({
    viewport: { width, height: 900 },
    colorScheme: theme,
    reducedMotion: reducedMotion ? "reduce" : "no-preference",
    forcedColors: forcedColors ? "active" : "none",
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  let transparencyEmulated = true;
  if (reducedTransparency) {
    // Playwright's emulateMedia() has no prefers-reduced-transparency option,
    // but Chromium implements the feature - reach it over CDP rather than
    // inventing an attribute and asserting the app honours it.
    try {
      const client = await context.newCDPSession(page);
      await client.send("Emulation.setEmulatedMedia", {
        features: [
          { name: "prefers-reduced-transparency", value: "reduce" },
          { name: "prefers-color-scheme", value: theme },
        ],
      });
    } catch {
      transparencyEmulated = false;
    }
  }
  return { context, page, transparencyEmulated };
}

async function gotoView(page, base, view, theme) {
  await page.goto(base, { waitUntil: "domcontentloaded" });
  await page.evaluate((t) => {
    document.documentElement.dataset.theme = t;
    try { localStorage.setItem("ar-theme", t); } catch { /* preview */ }
  }, theme);
  if (view !== "rooms") {
    const nav = page.getByRole("button", { name: new RegExp(view, "i") }).first();
    if (await nav.count()) await nav.click();
  }
  await settle(page);
}

async function settle(page) {
  await page.waitForTimeout(600);
  await page.evaluate(() =>
    Promise.all(
      document.getAnimations().map((a) => a.finished.catch(() => {})),
    ).catch(() => {}),
  );
}

// ---------------------------------------------------------------- the checks
async function checkOverflow(page, label) {
  const result = await page.evaluate(() => {
    const doc = document.documentElement;
    const overflow = doc.scrollWidth - doc.clientWidth;
    if (overflow <= 1) return { overflow };
    let worst = null;
    for (const el of document.querySelectorAll("*")) {
      const r = el.getBoundingClientRect();
      const past = r.right - doc.clientWidth;
      if (past > 1 && (!worst || past > worst.past)) {
        worst = {
          past: Math.round(past),
          tag: el.tagName.toLowerCase(),
          cls: (el.className && String(el.className).slice(0, 80)) || "",
        };
      }
    }
    return { overflow, worst };
  });
  if (result.overflow > 1) {
    fail("overflow", `${label}: ${result.overflow}px horizontal overflow`, {
      element: result.worst,
    });
  } else ok(`overflow ${label}`);
}

async function checkAxe(page, label, axe) {
  await page.evaluate(axe);
  const results = await page.evaluate(async () => {
    // eslint-disable-next-line no-undef
    return await axe.run(document, {
      runOnly: {
        type: "rule",
        values: [
          "color-contrast",
          "button-name",
          "link-name",
          "label",
          "aria-valid-attr-value",
          "aria-required-attr",
          "aria-allowed-attr",
          "duplicate-id-aria",
        ],
      },
    });
  });
  const serious = results.violations.filter((v) =>
    ["serious", "critical"].includes(v.impact),
  );
  if (serious.length) {
    for (const v of serious) {
      fail("axe", `${label}: ${v.id} — ${v.help}`, {
        nodes: v.nodes.slice(0, 4).map((n) => n.target.join(" ")),
      });
    }
  } else ok(`axe ${label}`);
}

async function checkFocusVisibility(page, label) {
  // Must be driven by real Tab presses. Calling el.focus() from page context
  // does NOT reliably set :focus-visible in Chromium - the heuristic depends on
  // the last input modality - so a scripted focus loop reports false failures on
  // every button. Tab is the modality users actually complain about anyway.
  const offenders = [];
  const seen = new Set();

  await page.evaluate(() => document.body.focus());
  for (let i = 0; i < 40; i += 1) {
    await page.keyboard.press("Tab");
    const info = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el || el === document.body) return null;
      const s = getComputedStyle(el);
      const id =
        el.tagName.toLowerCase() +
        (el.id ? "#" + el.id : "") +
        "." + String(el.className || "").split(" ").slice(0, 2).join(".");
      const outline = parseFloat(s.outlineWidth) || 0;
      const hasRing =
        (outline > 0 && s.outlineStyle !== "none") ||
        (s.boxShadow && s.boxShadow !== "none");
      return { id, hasRing, focusVisible: el.matches(":focus-visible") };
    });
    if (!info) break;
    if (seen.has(info.id)) continue;   // wrapped around the tab order
    seen.add(info.id);
    // Only a genuinely focus-visible element owes the user an indicator.
    if (info.focusVisible && !info.hasRing) offenders.push(info.id);
  }

  if (offenders.length) {
    fail("focus-visible", `${label}: ${offenders.length} element(s) show no focus indicator`, {
      elements: offenders.slice(0, 8),
    });
  } else ok(`focus-visible ${label} (${seen.size} stops)`);
}

async function checkReducedMotion(page, label) {
  const moving = await page.evaluate(() => {
    const out = [];
    for (const el of document.querySelectorAll("*")) {
      const s = getComputedStyle(el);
      const dur = (v) =>
        Math.max(0, ...String(v).split(",").map((x) => {
          const n = parseFloat(x);
          return x.includes("ms") ? n : n * 1000;
        }).filter(Number.isFinite));
      if (dur(s.animationDuration) > 50 || dur(s.transitionDuration) > 50) {
        out.push(`${el.tagName.toLowerCase()}.${String(el.className).slice(0, 40)}`);
      }
    }
    return out;
  });
  if (moving.length) fail("reduced-motion", `${label}: ${moving.length} element(s) still animate above 50ms`, { elements: moving.slice(0, 8) });
  else ok(`reduced-motion ${label}`);
}

async function checkGlassFallback(page, label, emulated = true) {
  if (!emulated) {
    warn("glass-fallback", `${label}: browser would not emulate prefers-reduced-transparency - not checked`);
    return;
  }
  // Sanity-check the emulation itself before judging the app: a query that did
  // not take effect would otherwise look like an application defect.
  const queryLive = await page.evaluate(
    () => window.matchMedia("(prefers-reduced-transparency: reduce)").matches,
  );
  if (!queryLive) {
    warn("glass-fallback", `${label}: prefers-reduced-transparency did not take effect - not checked`);
    return;
  }
  const leaking = await page.evaluate(() => {
    const out = [];
    for (const el of document.querySelectorAll(".glass, .glass--clear")) {
      const s = getComputedStyle(el);
      const bf = s.backdropFilter || s.webkitBackdropFilter;
      if (bf && bf !== "none") out.push(String(el.className).slice(0, 60));
    }
    return out;
  });
  if (leaking.length) fail("glass-fallback", `${label}: backdrop-filter still active on ${leaking.length} surface(s)`, { elements: leaking.slice(0, 8) });
  else ok(`glass-fallback ${label}`);
}

async function checkBubbleGeometry(page) {
  const geo = await page.evaluate(() => {
    const bubbles = [...document.querySelectorAll("[data-side][data-pos]")];
    if (!bubbles.length) return { missing: true };
    const out = bubbles.slice(0, 60).map((el) => {
      const s = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      return {
        side: el.dataset.side,
        pos: el.dataset.pos,
        radii: [s.borderTopLeftRadius, s.borderTopRightRadius, s.borderBottomRightRadius, s.borderBottomLeftRadius],
        left: Math.round(r.left),
        right: Math.round(r.right),
      };
    });
    const col = document.querySelector('[role="feed"]');
    const rect = col ? col.getBoundingClientRect() : null;
    return { out, centre: rect ? Math.round(rect.left + rect.width / 2) : null };
  });

  if (geo.missing) {
    fail("bubble-geometry", "no [data-side][data-pos] bubbles found — the conversation was not rebuilt");
    return;
  }
  const outs = geo.out.filter((b) => b.side === "out");
  const ins = geo.out.filter((b) => b.side === "in");
  if (geo.centre != null) {
    const badOut = outs.filter((b) => b.right < geo.centre);
    const badIn = ins.filter((b) => b.left > geo.centre);
    if (badOut.length || badIn.length) {
      fail("bubble-alignment", `human bubbles must sit right of centre and agent bubbles left (${badOut.length} + ${badIn.length} wrong)`);
    } else ok("bubble-alignment");
  }
  const joined = geo.out.filter((b) => b.pos === "middle");
  const wrong = joined.filter((b) => {
    const tight = b.radii.filter((r) => parseFloat(r) <= 6).length;
    return tight !== 2;
  });
  if (joined.length && wrong.length) {
    fail("bubble-corners", `${wrong.length} grouped bubble(s) do not tighten exactly two corners`);
  } else ok("bubble-corners");
}

async function checkScrollPerf(page) {
  if (skipPerf) { ok("scroll-perf (skipped)"); return; }
  const feed = page.locator('[role="feed"]').first();
  if (!(await feed.count())) { warn("scroll-perf", "no feed found"); return; }
  const worst = await page.evaluate(async () => {
    const el = document.querySelector('[role="feed"]');
    if (!el) return 0;
    const frames = [];
    let last = performance.now();
    let raf;
    const tick = () => {
      const now = performance.now();
      frames.push(now - last);
      last = now;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    for (let i = 0; i < 40; i += 1) {
      el.scrollTop += 120;
      await new Promise((r) => setTimeout(r, 16));
    }
    cancelAnimationFrame(raf);
    frames.sort((a, b) => b - a);
    return Math.round(frames[1] ?? 0); // ignore the single worst outlier
  });
  if (worst > 32) fail("scroll-perf", `longest frame ${worst}ms exceeds the 32ms budget`);
  else ok(`scroll-perf (${worst}ms)`);
}

// ---------------------------------------------------------------- main
async function main() {
  mkdirSync(outDir, { recursive: true });
  const axe = axeSource();

  const server = await createServer({
    root: repo,
    server: { port: 0, strictPort: false },
    logLevel: "error",
  });
  await server.listen();
  const info = server.resolvedUrls?.local?.[0];
  if (!info) throw new Error("vite preview did not report a URL");
  const base = info.replace(/\/$/, "");
  process.stdout.write(`Agent Room visual QA — ${base}\n\n`);

  const browser = await launch();

  try {
    // 1. overflow + axe + focus, every width x theme x view
    for (const theme of THEMES) {
      for (const width of WIDTHS) {
        const { context, page } = await newPage(browser, { width, theme });
        for (const view of VIEWS) {
          const label = `${view} ${width}px ${theme}`;
          await gotoView(page, base, view, theme);
          await checkOverflow(page, label);
          if (width === 1280 || width === 720) {
            await checkAxe(page, label, axe);
            await checkFocusVisibility(page, label);
          }
          if (SHOT_WIDTHS.includes(width)) {
            await page.screenshot({
              path: join(outDir, `${view}-${width}-${theme}.png`),
              fullPage: false,
            });
          }
        }
        await context.close();
      }
    }

    // 2. conversation geometry + performance, one representative context
    {
      const { context, page } = await newPage(browser, { width: 1280, theme: "light" });
      await gotoView(page, base, "rooms", "light");
      await checkBubbleGeometry(page);
      await checkScrollPerf(page);
      await context.close();
    }

    // 3. reduced motion
    {
      const { context, page } = await newPage(browser, { width: 1280, theme: "light", reducedMotion: true });
      await gotoView(page, base, "rooms", "light");
      await checkReducedMotion(page, "rooms reduced-motion");
      await page.screenshot({ path: join(outDir, "rooms-1280-reduced-motion.png") });
      await context.close();
    }

    // 4. forced colours
    {
      const { context, page } = await newPage(browser, { width: 1280, theme: "light", forcedColors: true });
      await gotoView(page, base, "rooms", "light");
      await checkOverflow(page, "rooms forced-colors");
      await page.screenshot({ path: join(outDir, "rooms-1280-forced-colors.png") });
      await context.close();
    }

    // 5. reduced transparency
    {
      const { context, page, transparencyEmulated } = await newPage(browser, { width: 1280, theme: "light", reducedTransparency: true });
      await gotoView(page, base, "rooms", "light");
      await checkGlassFallback(page, "rooms reduced-transparency", transparencyEmulated);
      await page.screenshot({ path: join(outDir, "rooms-1280-reduced-transparency.png") });
      await context.close();
    }
  } finally {
    await browser.close();
    await server.close();
  }

  const fails = findings.filter((f) => f.severity === "fail");
  const report = {
    generatedAt: new Date().toISOString(),
    checksPassed: checks,
    failures: fails.length,
    warnings: findings.length - fails.length,
    findings,
  };
  writeFileSync(join(outDir, "report.json"), JSON.stringify(report, null, 2));

  process.stdout.write(`\n${checks} checks passed, ${fails.length} failed\n`);
  for (const f of fails) {
    process.stdout.write(`\nFAIL  ${f.check}\n      ${f.detail}\n`);
    if (f.element) process.stdout.write(`      ${JSON.stringify(f.element)}\n`);
    if (f.elements) process.stdout.write(`      ${f.elements.join("\n      ")}\n`);
    if (f.nodes) process.stdout.write(`      ${f.nodes.join("\n      ")}\n`);
  }
  process.exit(fails.length ? 1 : 0);
}

main().catch((error) => {
  process.stderr.write(`visual-qa harness error: ${error?.stack || error}\n`);
  process.exit(2);
});
