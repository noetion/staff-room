#!/usr/bin/env node

import { chromium } from "@playwright/test";
import { createServer } from "vite";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "..", "..");
const output = resolve(process.argv[2] ?? resolve(repo, "docs", "assets", "demo-source"));

function installTauriDemoMock() {
  const callbacks = new Map();
  let callbackId = 0;

  const project = {
    id: "staff-room-demo",
    name: "The Staff Room",
    goal: "Coordinate coding agents without surrendering custody of the repository.",
    repositoryPath: "C:\\Projects\\fixture-repo",
    branch: "main",
  };

  const codex = {
    kind: "codex",
    name: "Codex",
    installed: true,
    version: "demo",
    state: "ready",
    connectionStatus: "connected",
    connectionDetail: "Synthetic demo provider",
    capabilities: {
      nonInteractiveTurn: true,
      streaming: true,
      structuredOutput: true,
      exactResume: true,
      cancellation: true,
      writeMode: true,
      approvalBridge: true,
      usageReporting: true,
      repositoryScoping: true,
      warmSession: true,
      autonomyMode: "isolated-auto",
      autonomyNote: "Every Ship run works in an isolated managed worktree.",
      capabilityProof: ["Synthetic demo provider"],
    },
    models: ["gpt-5.6-sol"],
    modelDiscoveryNote: "Synthetic demo catalogue",
    supportsEffort: true,
    effortOptions: ["high", "max", "ultra"],
  };

  const claude = {
    ...codex,
    kind: "claude",
    name: "Claude",
    connectionDetail: "Synthetic demo provider",
    capabilities: {
      ...codex.capabilities,
      autonomyNote: "Independent review runs in the same isolated worktree.",
    },
    models: ["claude-sonnet-4-6"],
    supportsEffort: false,
    effortOptions: [],
  };

  const cursor = {
    ...codex,
    kind: "cursor",
    name: "Cursor",
    connectionDetail: "Synthetic demo provider",
    capabilities: {
      ...codex.capabilities,
      autonomyNote: "Cursor participates through the same reviewed handoff contract.",
    },
    models: ["cursor-auto"],
    supportsEffort: false,
    effortOptions: [],
  };

  const antigravity = {
    ...codex,
    kind: "antigravity",
    name: "Antigravity",
    connectionDetail: "Synthetic demo provider",
    capabilities: {
      ...codex.capabilities,
      autonomyNote: "Antigravity participates through the same reviewed handoff contract.",
    },
    models: ["gemini-3.1-pro-high"],
    supportsEffort: false,
    effortOptions: [],
  };

  const baseMessages = [
    {
      id: "status-attached",
      kind: "status",
      sender: "system",
      body: "The synthetic release repository is attached and ready.",
      reason: "The Staff Room keeps every action scoped to C:\\Projects\\fixture-repo.",
      createdAt: "2026-08-08T09:00:00.000Z",
    },
    {
      id: "ask-human",
      kind: "human",
      sender: "human",
      body: "Where is the public release still exposed?",
      createdAt: "2026-08-08T09:00:01.000Z",
      runId: "chat-demo",
    },
    {
      id: "ask-agent",
      kind: "agent",
      sender: "codex",
      body: "Two blockers remain: sanitize repository history and verify the packaged Windows build. I have not changed the checkout.",
      createdAt: "2026-08-08T09:00:02.000Z",
      runId: "chat-demo",
    },
  ];

  const shipEvidence = {
    id: "ship-evidence",
    kind: "evidence",
    sender: "codex",
    body: "The release candidate is built in an isolated worktree. Tests and review are complete; promotion is waiting for you.",
    changedFiles: ["README.md", "SECURITY.md", "src-tauri/src/db/app_data.rs"],
    verification: [
      { label: "Frontend tests", status: "passed", detail: "24 passed" },
      { label: "Rust tests", status: "passed", detail: "63 passed" },
      { label: "Visual acceptance", status: "passed", detail: "89 checks passed" },
    ],
    createdAt: "2026-08-08T09:00:05.000Z",
    runId: "ship-demo",
  };

  function currentStage() {
    return new URL(window.location.href).searchParams.get("demoStage") ?? "ask";
  }

  function latestRun() {
    const stage = currentStage();
    if (stage !== "review" && stage !== "promotion") return undefined;
    const promoted = stage === "promotion";
    return {
      id: "ship-demo",
      objective: "Prepare The Staff Room for a safe public release.",
      state: promoted ? "awaiting-promotion" : "reviewing",
      currentOwner: promoted ? undefined : "claude",
      writer: "codex",
      reviewer: "claude",
      route: [
        { agent: "codex", label: "Build", state: "complete" },
        { agent: "codex", label: "Verify", state: "complete" },
        { agent: "claude", label: "Review", state: promoted ? "complete" : "current" },
        { label: "Promote", state: promoted ? "current" : "next" },
      ],
      reviewCount: promoted ? 2 : 1,
      revisionCount: 0,
      recoveryCount: 0,
      startedAt: "2026-08-08T09:00:03.000Z",
      worktreePath: "C:\\Projects\\fixture-repo\\.staff-room\\worktrees\\ship-demo",
      branch: "staff-room/public-release",
      contextBytes: 18432,
      degradedReview: false,
      instructionFiles: ["AGENTS.md"],
      skillFiles: ["ship/SKILL.md"],
    };
  }

  function snapshot() {
    const stage = currentStage();
    return {
      messages: stage === "ask" ? baseMessages : [...baseMessages, shipEvidence],
      hasMore: false,
      latestRun: latestRun(),
      receipts: [],
    };
  }

  async function invoke(command, args = {}) {
    switch (command) {
      case "plugin:event|listen":
        return args.handler;
      case "plugin:event|unlisten":
        return null;
      case "get_environment":
        return {
          native: true,
          attached: true,
          repositoryPath: project.repositoryPath,
          branch: project.branch,
          participants: [codex, claude, cursor, antigravity],
          contextBudgetBytes: 49152,
        };
      case "project_active":
        return project;
      case "project_list":
        return [project];
      case "load_room":
        return snapshot();
      case "load_provider_profiles":
        return [
          { participantKind: "codex", route: "chat", model: "gpt-5.6-sol", effort: "high" },
          { participantKind: "codex", route: "build", model: "gpt-5.6-sol", effort: "max" },
          { participantKind: "codex", route: "review", model: "gpt-5.6-sol", effort: "max" },
        ];
      case "load_project_settings":
        return { autonomousShipEnabled: true };
      case "load_verification_config":
        return {
          enabled: true,
          commands: [
            { label: "Frontend tests", command: "npm test", enabled: true },
            { label: "Rust tests", command: "cargo test", enabled: true },
          ],
        };
      case "voice_status":
        return { available: false, recording: false, detail: "Disabled for silent demo", maxSeconds: 30 };
      case "allocate_operation_id":
        return "quick-edit-demo";
      case "quick_edit_start":
        return {
          editId: "quick-edit-demo",
          participant: "codex",
          summary: "Tightened the README opening without touching implementation code.",
          diff: "--- a/README.md\n+++ b/README.md\n@@\n-Coordinate coding agents.\n+Coordinate coding agents without surrendering custody of your repository.",
          stopped: false,
        };
      case "start_room_chat":
        return { runId: "chat-demo", participant: "codex", summary: "Release exposure reviewed.", stopped: false };
      case "quick_edit_apply":
        return {};
      case "start_room_run":
        return {
          runId: "ship-demo",
          state: "awaiting-promotion",
          summary: "Release candidate verified and reviewed.",
          builder: "codex",
          reviewer: "claude",
          degradedReview: false,
          changedFiles: ["README.md"],
          gitStatus: "M README.md",
          verification: [],
          stopped: false,
          promoted: false,
          worktreePath: "C:\\Projects\\fixture-repo\\.staff-room\\worktrees\\ship-demo",
          branch: "staff-room/public-release",
          contextBytes: 18432,
          instructionFiles: ["AGENTS.md"],
          skillFiles: ["ship/SKILL.md"],
          recoveryCount: 0,
        };
      default:
        return null;
    }
  }

  function transformCallback(callback, once = false) {
    callbackId += 1;
    const id = callbackId;
    callbacks.set(id, (payload) => {
      if (once) callbacks.delete(id);
      return callback?.(payload);
    });
    return id;
  }

  window.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback,
    unregisterCallback: (id) => callbacks.delete(id),
    runCallback: (id, payload) => callbacks.get(id)?.(payload),
    callbacks,
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
    convertFileSrc: (path) => `http://asset.localhost/${encodeURIComponent(path)}`,
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event, id) => callbacks.delete(id),
  };
}

async function settle(page) {
  await page.waitForSelector("[role='feed']");
  await page.waitForTimeout(400);
  await page.evaluate(async () => {
    await document.fonts.ready;
    document.activeElement?.blur?.();
  });
}

async function main() {
  mkdirSync(output, { recursive: true });
  const server = await createServer({
    root: repo,
    server: { host: "127.0.0.1", port: 0, strictPort: false },
    logLevel: "error",
  });
  await server.listen();
  const base = server.resolvedUrls?.local?.[0]?.replace(/\/$/, "");
  if (!base) throw new Error("Vite did not report a preview URL");

  const browser = await chromium.launch({
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || process.env.CHROME_PATH || undefined,
    args: ["--force-color-profile=srgb", "--disable-lcd-text"],
  });
  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    colorScheme: "light",
    reducedMotion: "reduce",
    deviceScaleFactor: 1,
  });
  await context.addInitScript(installTauriDemoMock);
  const page = await context.newPage();

  try {
    await page.goto(`${base}/?demoStage=ask`, { waitUntil: "domcontentloaded" });
    await page.getByRole("button", { name: "Ship", exact: true }).click();
    await settle(page);
    await page.screenshot({ path: resolve(output, "01-ask.png") });

    await page.getByRole("button", { name: "Quick edit", exact: true }).click();
    await page.getByRole("combobox", { name: "Quick Edit instruction" }).fill("Tighten the README opening.");
    await page.getByRole("combobox", { name: "Quick Edit instruction" }).press("Enter");
    await page.getByRole("region", { name: "Quick Edit review" }).waitFor();
    await settle(page);
    await page.screenshot({ path: resolve(output, "02-quick-edit.png") });

    await page.goto(`${base}/?demoStage=review`, { waitUntil: "domcontentloaded" });
    await settle(page);
    await page.screenshot({ path: resolve(output, "03-review.png") });

    await page.goto(`${base}/?demoStage=promotion`, { waitUntil: "domcontentloaded" });
    await settle(page);
    await page.screenshot({ path: resolve(output, "04-awaiting-promotion.png") });
  } finally {
    await context.close();
    await browser.close();
    await server.close();
  }

  process.stdout.write(`Captured Staff Room demo states in ${output}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error?.stack || error}\n`);
  process.exit(1);
});
