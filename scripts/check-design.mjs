#!/usr/bin/env node
/**
 * Agent Room - design token lint.
 *
 * Enforces one rule: design values live in src/design/, everything else
 * references them. It flags RAW values only. A declaration that resolves
 * through a token - `font-family: var(--font-mono)`, `border-radius:
 * calc(var(--r-xl) - var(--s-3))` - is exactly what the design system asks for
 * and must pass.
 *
 *   node scripts/check-design.mjs
 *   node scripts/check-design.mjs --list   (show what it scanned)
 *
 * Exit 0 clean, 1 on violations.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(repo, "src");

// Where design values are allowed to be literal.
const TOKEN_HOME = ["src/design"];

// Files the redesign has not reached yet. Step 15 deletes src/styles.css and
// this entry with it; if the array is empty the lint is fully enforced.
const LEGACY = ["src/styles.css"];

const SKIP_DIRS = new Set(["node_modules", "dist", ".git", "target", "coverage"]);
const EXTS = new Set([".css", ".tsx", ".ts"]);

const args = new Set(process.argv.slice(2));
const violations = [];
const scanned = [];

// ---------------------------------------------------------------- helpers
function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else if (EXTS.has(name.slice(name.lastIndexOf(".")))) out.push(full);
  }
  return out;
}

const norm = (p) => relative(repo, p).split(sep).join("/");
const exempt = (rel) =>
  TOKEN_HOME.some((d) => rel.startsWith(d + "/")) || LEGACY.includes(rel);

function stripComments(text) {
  // Replace comment bodies with spaces so line/column numbers stay intact.
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/(^|[^:])\/\/[^\n]*/g, (m, p) => p + " ".repeat(m.length - p.length));
}

function lineOf(text, index) {
  return text.slice(0, index).split("\n").length;
}

function add(rel, line, rule, snippet) {
  violations.push({ rel, line, rule, snippet: snippet.trim().slice(0, 100) });
}

// A value that goes through a token is the goal, not a violation.
const tokenised = (value) => /var\(\s*--/.test(value);

// ---------------------------------------------------------------- rules
const HEX = /#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;
const FUNC_COLOR = /\b(?:rgba?|hsla?)\(\s*[\d.]/; // literal numbers, not rgb(from ...)
const RAW_RADIUS = /(?:^|\s)(?:-?\d*\.?\d+)(?:px|rem|em)\b/;
const BEZIER = /cubic-bezier\(/;
const RAW_TIME = /(?:^|\s)(?:-?\d*\.?\d+)m?s\b/;

function checkCss(rel, text) {
  const clean = stripComments(text);

  // Colour literals are never acceptable outside src/design, tokenised or not.
  for (const re of [HEX, FUNC_COLOR]) {
    const g = new RegExp(re.source, "g");
    let m;
    while ((m = g.exec(clean))) {
      // url(#filter-id) is a reference, not a colour.
      const before = clean.slice(Math.max(0, m.index - 5), m.index);
      if (/url\($/.test(before)) continue;
      add(rel, lineOf(clean, m.index), "raw colour", m[0]);
    }
  }

  // Declaration-level rules: skip anything already resolved through a token.
  const decl = /([-a-zA-Z]+)\s*:\s*([^;{}]+)[;}]/g;
  let d;
  while ((d = decl.exec(clean))) {
    const prop = d[1].toLowerCase();
    const value = d[2];
    if (tokenised(value)) continue;
    const line = lineOf(clean, d.index);

    if (prop === "font-family") add(rel, line, "raw font stack", d[0]);
    if (prop === "font" && /["']|\b(serif|sans-serif|monospace)\b/.test(value))
      add(rel, line, "raw font stack", d[0]);
    if (prop === "border-radius" && RAW_RADIUS.test(value))
      add(rel, line, "raw radius", d[0]);
    if ((prop === "transition-duration" || prop === "animation-duration") && RAW_TIME.test(value))
      add(rel, line, "raw duration", d[0]);
    if (/^(transition|animation)$/.test(prop) && RAW_TIME.test(value))
      add(rel, line, "raw duration", d[0]);
    if (BEZIER.test(value)) add(rel, line, "raw easing", d[0]);
    if (prop === "transition-timing-function" || prop === "animation-timing-function") {
      if (!/^\s*(linear|ease|ease-in|ease-out|ease-in-out|step[s-][^;]*)\s*$/.test(value))
        add(rel, line, "raw easing", d[0]);
    }
  }
}

function checkTs(rel, text) {
  // In TS/TSX only literal colours are worth policing. Anything structural
  // belongs in CSS anyway, and parsing JSX for style objects produces noise.
  const clean = stripComments(text);
  for (const re of [HEX, FUNC_COLOR]) {
    const g = new RegExp(re.source, "g");
    let m;
    while ((m = g.exec(clean))) {
      const before = clean.slice(Math.max(0, m.index - 5), m.index);
      if (/url\($/.test(before)) continue;
      // Ignore ids and anchors: href="#top", getElementById("#x")
      const after = clean.slice(m.index, m.index + 40);
      if (/^#[0-9a-fA-F]{3,8}[a-zA-Z_-]/.test(after)) continue;
      add(rel, lineOf(clean, m.index), "raw colour", m[0]);
    }
  }
}

// ---------------------------------------------------------------- run
let files = [];
try {
  files = walk(SRC);
} catch {
  process.stderr.write("no src/ directory found\n");
  process.exit(1);
}

for (const file of files) {
  const rel = norm(file);
  if (exempt(rel)) continue;
  scanned.push(rel);
  const text = readFileSync(file, "utf8");
  if (rel.endsWith(".css")) checkCss(rel, text);
  else checkTs(rel, text);
}

if (args.has("--list")) {
  process.stdout.write(`scanned ${scanned.length} file(s):\n`);
  scanned.forEach((f) => process.stdout.write(`  ${f}\n`));
  if (LEGACY.length) process.stdout.write(`skipped (legacy): ${LEGACY.join(", ")}\n`);
}

if (!violations.length) {
  process.stdout.write(`design lint clean (${scanned.length} files)\n`);
  process.exit(0);
}

for (const v of violations) {
  process.stdout.write(`${v.rel}:${v.line}  ${v.rule}: ${v.snippet}\n`);
}
process.stdout.write(`\n${violations.length} design violation(s)\n`);
process.stdout.write(
  "Raw design values belong in src/design/. Reference them with var(--token).\n",
);
process.exit(1);
