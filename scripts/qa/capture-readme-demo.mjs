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
    id: "checkout-service-demo",
    name: "Checkout Service",
    goal: "Keep checkout reliable under retries and concurrent requests.",
    repositoryPath: "C:\\Projects\\checkout-service",
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
      body: "Checkout Service is attached and ready.",
      reason: "The Staff Room keeps every action scoped to C:\\Projects\\checkout-service.",
      createdAt: new Date(Date.now() - 30_000).toISOString(),
    },
    {
      id: "diagnose-human",
      kind: "human",
      sender: "human",
      body: "@claude Checkout retries are double-charging customers. Find the failure mode before we change anything.",
      createdAt: new Date(Date.now() - 20_000).toISOString(),
      runId: "chat-demo",
    },
    {
      id: "diagnose-agent",
      kind: "agent",
      sender: "claude",
      body: "The handler creates the charge before it atomically claims the idempotency key. Two concurrent retries can both pass the lookup. Claim the key first, then return the original payment for duplicates.",
      createdAt: new Date(Date.now() - 18_000).toISOString(),
      runId: "chat-demo",
    },
  ];

  const cursorMessages = [
    ...baseMessages,
    {
      id: "cursor-human",
      kind: "human",
      sender: "human",
      body: "@cursor Add one regression test that sends two concurrent requests with the same idempotency key and proves only one charge is created.",
      createdAt: new Date(Date.now() - 14_000).toISOString(),
      runId: "quick-edit-demo",
    },
  ];

  const shipEvidence = {
    id: "ship-evidence",
    kind: "evidence",
    sender: "antigravity",
    body: "Payment creation is now idempotent. Retry coverage passes; the attached checkout is unchanged until you promote.",
    changedFiles: [
      "src/payments/create-payment.ts",
      "src/payments/idempotency-store.ts",
      "tests/payments/retry-idempotency.test.ts",
    ],
    verification: [
      { label: "Payment regression", status: "passed", detail: "passed" },
      { label: "Concurrent retries", status: "passed", detail: "one charge created" },
      { label: "Full test suite", status: "passed", detail: "passed" },
    ],
    createdAt: new Date(Date.now() - 5_000).toISOString(),
    runId: "ship-demo",
  };

  function currentStage() {
    return new URL(window.location.href).searchParams.get("demoStage") ?? "ask";
  }

  function latestRun() {
    const stage = currentStage();
    if (stage !== "build" && stage !== "review" && stage !== "promotion") return undefined;
    const building = stage === "build";
    const promoted = stage === "promotion";
    return {
      id: "ship-demo",
      objective: "Make payment creation idempotent and prove it with a regression test.",
      state: promoted ? "awaiting-promotion" : building ? "working" : "reviewing",
      currentOwner: promoted ? undefined : building ? "antigravity" : "codex",
      writer: "antigravity",
      reviewer: "codex",
      route: [
        { agent: "antigravity", label: "Build", state: building ? "current" : "complete" },
        { agent: "antigravity", label: "Verify", state: building ? "next" : "complete" },
        { agent: "codex", label: "Review", state: promoted ? "complete" : building ? "next" : "current" },
        { label: "Promote", state: promoted ? "current" : "next" },
      ],
      reviewCount: promoted ? 2 : building ? 0 : 1,
      revisionCount: 0,
      recoveryCount: 0,
      startedAt: new Date(Date.now() - 12_000).toISOString(),
      worktreePath: "C:\\Projects\\checkout-service\\.staff-room\\worktrees\\ship-demo",
      branch: "staff-room/payment-idempotency",
      contextBytes: 18432,
      degradedReview: false,
      instructionFiles: ["AGENTS.md"],
      skillFiles: ["ship/SKILL.md"],
    };
  }

  function snapshot() {
    const stage = currentStage();
    return {
      messages: stage === "cursor"
        ? cursorMessages
        : stage === "review" || stage === "promotion"
          ? [...cursorMessages, shipEvidence]
          : baseMessages,
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
            { label: "Payment tests", command: "npm test -- payments", enabled: true },
            { label: "Full test suite", command: "npm test", enabled: true },
          ],
        };
      case "voice_status":
        return { available: false, recording: false, detail: "Disabled for silent demo", maxSeconds: 30 };
      case "allocate_operation_id":
        return "quick-edit-demo";
      case "quick_edit_start":
        return {
          editId: "quick-edit-demo",
          participant: "cursor",
          summary: "Added a focused concurrency regression test. It fails until payment creation claims the idempotency key atomically.",
          diff: "--- /dev/null\n+++ b/tests/payments/retry-idempotency.test.ts\n@@\n+it('creates one charge for concurrent retries', async () => {\n+  const [first, retry] = await Promise.all([\n+    createPayment(request, 'checkout-42'),\n+    createPayment(request, 'checkout-42'),\n+  ]);\n+  expect(retry.id).toBe(first.id);\n+  expect(gateway.charges).toHaveLength(1);\n+});",
          stopped: false,
        };
      case "start_room_chat":
        return { runId: "chat-demo", participant: "codex", summary: "Payment retry path reviewed.", stopped: false };
      case "quick_edit_apply":
        return {};
      case "start_room_run":
        return {
          runId: "ship-demo",
          state: "awaiting-promotion",
          summary: "Idempotent payment creation verified and reviewed.",
          builder: "antigravity",
          reviewer: "codex",
          degradedReview: false,
          changedFiles: [
            "src/payments/create-payment.ts",
            "src/payments/idempotency-store.ts",
            "tests/payments/retry-idempotency.test.ts",
          ],
          gitStatus: "M src/payments/create-payment.ts\nA src/payments/idempotency-store.ts\nA tests/payments/retry-idempotency.test.ts",
          verification: [],
          stopped: false,
          promoted: false,
          worktreePath: "C:\\Projects\\checkout-service\\.staff-room\\worktrees\\ship-demo",
          branch: "staff-room/payment-idempotency",
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
    await settle(page);
    await page.screenshot({ path: resolve(output, "01-claude-diagnosis.png") });

    await page.goto(`${base}/?demoStage=cursor`, { waitUntil: "domcontentloaded" });
    await page.getByRole("button", { name: "Quick edit", exact: true }).click();
    await page.getByRole("button", { name: "Cursor", exact: true }).click();
    await page.getByRole("combobox").fill("@cursor Add the focused concurrent-retry regression test.");
    await page.getByRole("button", { name: /Preview edit/ }).click();
    await page.getByRole("region", { name: "Quick Edit review" }).waitFor();
    await settle(page);
    await page.screenshot({ path: resolve(output, "02-cursor-test.png") });

    await page.goto(`${base}/?demoStage=build`, { waitUntil: "domcontentloaded" });
    await settle(page);
    await page.screenshot({ path: resolve(output, "03-antigravity-build.png") });

    await page.goto(`${base}/?demoStage=review`, { waitUntil: "domcontentloaded" });
    await settle(page);
    await page.screenshot({ path: resolve(output, "04-codex-review.png") });

    await page.goto(`${base}/?demoStage=promotion`, { waitUntil: "domcontentloaded" });
    await settle(page);
    await page.screenshot({ path: resolve(output, "05-awaiting-promotion.png") });
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
