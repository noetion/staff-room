#!/usr/bin/env node
/**
 * The Staff Room — automated visual QA harness.
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

// One representative selector per component stylesheet. If a sheet is never
// imported its rules are simply absent - no error, no failed request, no
// visual failure. This is the only check that notices.
const REQUIRED_SELECTORS = [
  ".primitive-segmented",   // primitives.css
  ".chrome-titlebar",       // chrome.css
  ".message-bubble",        // conversation.css
  ".composer",              // composer.css
  ".inspector-sheet",       // inspector.css
  ".settings-view",         // settings.css
];

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
    try { localStorage.setItem("staff-room-theme", t); } catch { /* preview */ }
  }, theme);
  if (view !== "rooms") {
    const nav = page.getByRole("button", { name: new RegExp(view, "i") }).first();
    if (await nav.count()) await nav.click();
  }
  await settle(page);
}

async function settle(page) {
  await page.waitForTimeout(600);
  await page.evaluate(() => {
    const finiteAnimations = document
      .getAnimations()
      .filter((animation) => Number.isFinite(animation.effect?.getComputedTiming().endTime));
    return Promise.race([
      Promise.all(finiteAnimations.map((animation) => animation.finished.catch(() => {}))),
      new Promise((resolve) => setTimeout(resolve, 1_200)),
    ]);
  });
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

  // Never discard these again. axe reports a case it cannot resolve as
  // `incomplete`, and a coloured parent with a re-coloured descendant is
  // precisely such a case. checkTextContrast() is the hard gate; these are the
  // pointers.
  for (const v of results.incomplete || []) {
    warn("axe-incomplete", `${label}: ${v.id} needs review — ${v.help}`, {
      nodes: v.nodes.slice(0, 3).map((n) => n.target.join(" ")),
    });
  }
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

async function checkStylesheetsLoaded(page, label, required) {
  // A component stylesheet that is never imported produces no error, no failed
  // request and no visual test failure - the components just render unstyled.
  // Assert the rules are actually in the document.
  const missing = await page.evaluate((selectors) => {
    const present = new Set();
    for (const sheet of document.styleSheets) {
      let rules;
      try { rules = sheet.cssRules; } catch { continue; }
      const walk = (list) => {
        for (const rule of list) {
          if (rule.selectorText) present.add(rule.selectorText);
          if (rule.cssRules) walk(rule.cssRules);
        }
      };
      walk(rules);
    }
    const joined = [...present].join(" | ");
    return selectors.filter((sel) => !joined.includes(sel));
  }, required);

  if (missing.length) {
    fail("stylesheets", `${label}: ${missing.length} stylesheet(s) not loaded`, {
      elements: missing,
    });
  } else ok(`stylesheets ${label}`);
}

async function checkTextContrast(page, label) {
  // Deterministic, and independent of axe's confidence. For every visible text
  // node, composite the real painted background down the ancestor chain and
  // measure. This is the check that catches a bubble whose descendant resets
  // its own colour back to the page ink.
  const bad = await page.evaluate(() => {
    const lum = (c) => {
      const f = (v) => {
        v /= 255;
        return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
      };
      return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
    };
    const parse = (s) => {
      const m = String(s).match(/rgba?\(([^)]+)\)/);
      if (!m) return null;
      const p = m[1].split(",").map((x) => parseFloat(x));
      return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
    };
    const over = (fg, bg) => {
      const a = fg[3];
      return [
        fg[0] * a + bg[0] * (1 - a),
        fg[1] * a + bg[1] * (1 - a),
        fg[2] * a + bg[2] * (1 - a),
        1,
      ];
    };
    const paintedBg = (el) => {
      let acc = null;
      let node = el;
      while (node && node !== document.documentElement.parentNode) {
        const c = parse(getComputedStyle(node).backgroundColor);
        if (c && c[3] > 0) acc = acc ? over(acc, c) : c;
        if (acc && acc[3] >= 1) return acc;
        node = node.parentElement;
      }
      return acc || [255, 255, 255, 1];
    };
    const ratio = (a, b) => {
      const [x, y] = [lum(a), lum(b)].sort((m, n) => n - m);
      return (x + 0.05) / (y + 0.05);
    };

    const out = [];
    for (const el of document.querySelectorAll("*")) {
      if (el.closest(":disabled, [aria-disabled='true']")) continue;
      const hasOwnText = [...el.childNodes].some(
        (n) => n.nodeType === 3 && n.textContent.trim().length > 1,
      );
      if (!hasOwnText) continue;
      const s = getComputedStyle(el);
      if (s.visibility === "hidden" || s.display === "none" || parseFloat(s.opacity) === 0) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2) continue;

      const fg = parse(s.color);
      if (!fg) continue;
      const bg = paintedBg(el);
      const composited = fg[3] < 1 ? over(fg, bg) : fg;
      const size = parseFloat(s.fontSize);
      const bold = parseInt(s.fontWeight, 10) >= 700;
      const large = size >= 24 || (size >= 18.66 && bold);
      const need = large ? 3 : 4.5;
      const got = ratio(composited, bg);
      if (got < need) {
        out.push({
          sel: el.tagName.toLowerCase() + "." + String(el.className || "").split(" ")[0],
          ratio: Math.round(got * 100) / 100,
          need,
          text: el.textContent.trim().slice(0, 40),
        });
      }
    }
    return out;
  });

  if (bad.length) {
    fail("text-contrast", `${label}: ${bad.length} text element(s) below threshold`, {
      elements: bad.slice(0, 8).map((b) => `${b.sel} ${b.ratio}:1 (needs ${b.need}) "${b.text}"`),
    });
  } else ok(`text-contrast ${label}`);
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

async function checkLaunchReadinessLayout(page) {
  const metrics = await page.evaluate(() => {
    const feed = document.querySelector('[role="feed"]');
    const conversation = document.querySelector('.conversation-column');
    const conversationScroller = document.querySelector('.conversation');
    const room = document.querySelector('.room');
    const composer = document.querySelector('.composer');
    const search = document.querySelector('.chrome-search-button');
    const documentHeight = Math.max(document.documentElement.scrollHeight, document.body.scrollHeight);
    const viewportHeight = window.innerHeight;
    const initialScrollY = window.scrollY;
    window.scrollTo(0, documentHeight);
    const afterScrollY = window.scrollY;
    window.scrollTo(0, initialScrollY);
    return {
      documentOverflow: documentHeight - viewportHeight,
      afterScrollY,
      feedCanScroll: Boolean(feed && feed.scrollHeight > feed.clientHeight),
      conversationOverflowY: conversationScroller ? getComputedStyle(conversationScroller).overflowY : '',
      overscrollBehaviorY: conversationScroller ? getComputedStyle(conversationScroller).overscrollBehaviorY : '',
      roomOverflow: room ? getComputedStyle(room).overflow : '',
      conversationWidth: conversation?.getBoundingClientRect().width ?? 0,
      composerWidth: composer?.getBoundingClientRect().width ?? 0,
      searchWidth: search?.getBoundingClientRect().width ?? 0,
    };
  });
  if (metrics.documentOverflow > 1 || metrics.afterScrollY > 1) {
    fail('room-scroll-boundary', `the document can scroll beyond the room (${Math.round(metrics.documentOverflow)}px)`);
  } else if (!['auto', 'scroll'].includes(metrics.conversationOverflowY)
    || metrics.overscrollBehaviorY !== 'contain'
    || metrics.roomOverflow !== 'hidden') {
    fail('room-scroll-boundary', 'the room/conversation scroll ownership contract is incomplete');
  } else ok('room-scroll-boundary');
  if (metrics.conversationWidth < 900 || metrics.composerWidth < 900) {
    fail('wide-room-content', `wide room content remains capped at ${Math.round(Math.min(metrics.conversationWidth, metrics.composerWidth))}px`);
  } else ok('wide-room-content');
  if (metrics.searchWidth < 380) fail('prominent-search', `search control is only ${Math.round(metrics.searchWidth)}px wide`);
  else ok('prominent-search');
}

async function checkInspectorEvidence(page) {
  const errors = [];
  const onPageError = (error) => errors.push(error.message);
  page.on('pageerror', onPageError);
  await page.getByRole('button', { name: 'Open context' }).click();
  await page.getByRole('tab', { name: 'Evidence' }).click();
  const panel = page.locator('.inspector-body');
  const rendered = await panel.getByText('Coordinator-owned evidence.').count();
  const visible = await page.getByRole('dialog').isVisible();
  await page.locator('.primitive-sheet__close').click();
  page.off('pageerror', onPageError);
  if (errors.length || !visible || !rendered) {
    fail('evidence-tab', errors[0] || 'Evidence tab did not remain mounted');
  } else ok('evidence-tab');
}

async function checkSettingsLayout(page) {
  const metrics = await page.evaluate(() => {
    const view = document.querySelector('.settings-view');
    const cards = [...document.querySelectorAll('.provider-profile-card')];
    const rows = new Set(cards.map((card) => Math.round(card.getBoundingClientRect().top)));
    return { width: view?.getBoundingClientRect().width ?? 0, cards: cards.length, rows: rows.size };
  });
  if (metrics.width < 1200 || metrics.rows > 1) {
    fail('settings-layout', `settings uses ${Math.round(metrics.width)}px across ${metrics.rows} provider rows`);
  } else ok('settings-layout');
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
  process.stdout.write(`The Staff Room visual QA — ${base}\n\n`);

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
            await checkTextContrast(page, label);
            await checkStylesheetsLoaded(page, label, REQUIRED_SELECTORS);
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
      await checkInspectorEvidence(page);
      await checkScrollPerf(page);
      await checkLaunchReadinessLayout(page);
      await context.close();
    }

    // 2b. Wide settings should use the available desktop measure.
    {
      const { context, page } = await newPage(browser, { width: 1800, theme: "light" });
      await gotoView(page, base, "settings", "light");
      await checkSettingsLayout(page);
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
