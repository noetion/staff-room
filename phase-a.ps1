# phase-a.ps1 - The Staff Room acceptance: setup S1-S3 + Phase A
#
#   .\phase-a.ps1
#
# Automates what a machine can check, prompts for what needs eyes,
# and writes docs/V1_ACCEPTANCE_PHASE_A.md.
# Costs zero provider tokens.

param(
  [switch]$SkipFixture,
  [switch]$SeedMessages,
  [string]$FixturePath = (Join-Path $env:TEMP "staff-room-fixture")
)

$ErrorActionPreference = "Continue"

$repo   = Split-Path -Parent $PSCommandPath
$fix    = [System.IO.Path]::GetFullPath($FixturePath)
$out    = Join-Path $repo "docs\V1_ACCEPTANCE_PHASE_A.md"
$rows   = @()

function Ask($id, $question) {
  Write-Host ""
  Write-Host "[$id] $question" -ForegroundColor Cyan
  do { $a = (Read-Host "  pass / fail / skip").Trim().ToLower() }
  while ($a -notin @("pass","fail","skip","p","f","s"))
  switch ($a[0]) { "p" { "PASS" } "f" { "FAIL" } default { "SKIP" } }
}
function Note($id, $result, $evidence) {
  $script:rows += [pscustomobject]@{ Id = $id; Result = $result; Evidence = $evidence }
  $c = @{ PASS="Green"; FAIL="Red" }[$result]; if (-not $c) { $c = "DarkGray" }
  Write-Host ("  -> {0}" -f $result) -ForegroundColor $c
}

Write-Host "The Staff Room - Phase A acceptance (zero provider tokens)" -ForegroundColor White
Write-Host ("=" * 60) -ForegroundColor DarkGray

# ---------------------------------------------------------------- locate DB
$dbCandidates = @(
  "$env:APPDATA\com.staffroom.desktop\staff-room.db",
  "$env:LOCALAPPDATA\com.staffroom.desktop\staff-room.db"
)
$db = $dbCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($db) { Write-Host "Database: $db" -ForegroundColor DarkGray }
else     { Write-Host "Database not found - launch the app once first." -ForegroundColor Yellow }

$worktreeRoot = "$env:LOCALAPPDATA\com.staffroom.desktop\worktrees"

# ---------------------------------------------------------------- S1 fixture
if (-not $SkipFixture) {
  Write-Host "`nS1  building fixture repo at $fix" -ForegroundColor White
  if (Test-Path $fix) { throw "Fixture path already exists. Remove it manually or pass a new -FixturePath." }
  New-Item -ItemType Directory -Force $fix | Out-Null
  Push-Location $fix
  git init -q
  @'
{
  "name": "staff-room-fixture",
  "version": "1.0.0",
  "type": "module",
  "scripts": { "test": "node test.js" }
}
'@ | Set-Content package.json -Encoding utf8
  "export function greet(name) { return 'Hello ' + name; }" | Set-Content greet.js -Encoding utf8
  "import { greet } from './greet.js'; if (greet('x') !== 'Hello x') { process.exit(1); } console.log('ok');" | Set-Content test.js -Encoding utf8
  "# Fixture`n`nKeep changes minimal. Do not add dependencies." | Set-Content AGENTS.md -Encoding utf8
  git add -A 2>$null
  git -c user.email=a@b.c -c user.name=fixture commit -qm "fixture"
  $testOk = $false
  try { node test.js | Out-Null; $testOk = ($LASTEXITCODE -eq 0) } catch { }
  Pop-Location
  Write-Host ("  npm test runs clean: {0}" -f $testOk) -ForegroundColor DarkGray
  if (-not $testOk) { Write-Host "  fixture test did not pass - check Node is on PATH" -ForegroundColor Yellow }
}

# ---------------------------------------------------------------- S3 baseline
Write-Host "`nS3  provider process baseline" -ForegroundColor White
$procs = Get-Process | Where-Object { $_.Name -match 'codex|claude|cursor|agy' }
if ($procs) {
  Write-Host "  WARNING - provider processes already running:" -ForegroundColor Yellow
  $procs | Select-Object Name, Id | Format-Table | Out-String | Write-Host
} else { Write-Host "  clean - no provider processes" -ForegroundColor DarkGray }

# ------------------------------------------------- A3 automated: dev paths
Write-Host "`n[3] scanning for developer absolute paths" -ForegroundColor White
$leakTargets = @("$repo\dist", "$repo\src", "$repo\src-tauri\src")
$leaks = @()
foreach ($t in $leakTargets) {
  if (Test-Path $t) {
    $leaks += Get-ChildItem $t -Recurse -File -Include *.js,*.ts,*.tsx,*.rs,*.html,*.css -ErrorAction SilentlyContinue |
      Select-String -Pattern 'C:\\+Users\\+[^\\]+' -List -ErrorAction SilentlyContinue
  }
}
if ($leaks) {
  Write-Host "  found in:" -ForegroundColor Yellow
  $leaks | ForEach-Object { Write-Host ("    " + $_.Path) -ForegroundColor Yellow }
  Note 3 "FAIL" ("developer path present in " + $leaks.Count + " file(s): " + (($leaks | ForEach-Object { Split-Path $_.Path -Leaf }) -join ", "))
} else {
  Note 3 "PASS" "no absolute Windows user path found in dist, src, or src-tauri/src"
}

# ------------------------------------------------- A25 opt-in: seed 500 into the exact fixture only
Write-Host "`n[25] 500-message room" -ForegroundColor White
$seeded = $false
if (-not $SeedMessages) {
  Write-Host "  disabled by default; attach the fixture, close the app, then rerun with -SkipFixture -SeedMessages" -ForegroundColor DarkGray
} elseif ($db) {
  $seedJs = Join-Path $env:TEMP "staff-room-seed.mjs"
  @'
import { DatabaseSync } from "node:sqlite";
import path from "node:path";
const db = new DatabaseSync(process.argv[2]);
const normalize = (value) => path.resolve(value.replace(/^\\\\\?\\/, "")).replaceAll("/", "\\").toLowerCase();
const expected = normalize(process.argv[3]);
const p = db.prepare("SELECT id, repository_path FROM projects").all()
  .find((project) => normalize(project.repository_path) === expected);
if (!p) { console.log("NOFIXTURE"); process.exit(2); }
const ins = db.prepare(
  "INSERT INTO messages (id, project_id, run_id, sender_kind, message_kind, body, created_at) VALUES (?,?,?,?,?,?,?)"
);
const base = Date.now() - 500 * 60000;
for (let i = 0; i < 500; i++) {
  const t = new Date(base + i * 60000).toISOString().replace("T", " ").slice(0, 19);
  ins.run(`seed-${i}-${Math.random().toString(16).slice(2)}`, p.id, null, "human", "human", `Seeded acceptance message ${i + 1} of 500.`, t);
}
console.log("OK");
'@ | Set-Content $seedJs -Encoding utf8
  $r = & node --experimental-sqlite $seedJs $db $fix 2>&1
  if ($r -match "OK") { $seeded = $true; Write-Host "  inserted 500 rows into the exact fixture project" -ForegroundColor DarkGray }
  elseif ($r -match "NOFIXTURE") { Write-Host "  exact fixture is not attached - attach $fix, close the app, then rerun with -SkipFixture -SeedMessages" -ForegroundColor Yellow }
  else { Write-Host "  could not seed (needs Node 22+ for node:sqlite): $r" -ForegroundColor Yellow }
} else {
  Write-Host "  database not found; launch the app once, attach the fixture, close it, then rerun with -SkipFixture -SeedMessages" -ForegroundColor Yellow
}

# ------------------------------------------------- A18 worktree inventory
$wtBefore = @()
if (Test-Path $worktreeRoot) { $wtBefore = Get-ChildItem $worktreeRoot -Directory | Select-Object -Expand Name }
Write-Host ("`n[18] worktrees present before: {0}" -f ($(if ($wtBefore) { $wtBefore -join ", " } else { "none" }))) -ForegroundColor White

# ---------------------------------------------------------------- manual
Write-Host "`n" ("=" * 60) -ForegroundColor DarkGray
Write-Host "MANUAL CHECKS - launch the app now:  npm run tauri dev" -ForegroundColor White
Write-Host ("=" * 60) -ForegroundColor DarkGray
Read-Host "`nPress Enter once the app is running"

# A6 - cold start probe cost
Write-Host "`n[6] sampling for provider subprocesses during startup..." -ForegroundColor Cyan
$spawned = @()
1..10 | ForEach-Object {
  $spawned += Get-Process | Where-Object { $_.Name -match 'codex|claude|cursor|agy' } | Select-Object -Expand Name
  Start-Sleep -Milliseconds 400
}
$spawned = $spawned | Select-Object -Unique
if ($spawned) { Note 6 "FAIL" ("provider processes spawned at startup: " + ($spawned -join ", ")) }
else          { Note 6 "PASS" "no provider subprocesses observed during startup" }

Note  1 (Ask  1 "Fresh launch shows an attach screen with NO seeded conversation? (rename the db aside first if you want a true cold start)") "observed"
Note  4 (Ask  4 "Prior messages and runs are intact after relaunch, no migration error?") "observed"
Note  2 (Ask  2 "Attach $fix as a second project. Do the two rooms have separate history, sessions, model profiles and settings?") "observed"
Note 29 (Ask 29 "Open the composer participant list. Is Antigravity ABSENT from Ask and Quick Edit, and does its card read 'Ship only'?") "observed"
Note 28 (Ask 28 "Resize to 1024, 1280, 1440 and 1800 px. No horizontal overflow at any width?") "observed"
if ($seeded) {
  Note 25 (Ask 25 "Reload the fixture room. Are the MOST RECENT messages shown (not the oldest 500), and does it scroll smoothly?") "500 synthetic messages inserted into the exact fixture project"
} else {
  Note 25 "SKIP" "message seeding was not requested or the exact fixture project could not be resolved"
}
Note 27 (Ask 27 "Look at any failed run or failed provider card. Does every failure name a cause AND offer an action?") "observed"
Note 18 (Ask 18 "If a non-active run with a worktree exists, click Abandon. Worktree removed and run marked abandoned? (skip if none)") "observed"

# A18 verification
if (Test-Path $worktreeRoot) {
  $wtAfter = Get-ChildItem $worktreeRoot -Directory | Select-Object -Expand Name
  $gone = $wtBefore | Where-Object { $_ -notin $wtAfter }
  if ($gone) { Write-Host ("  worktrees removed: " + ($gone -join ", ")) -ForegroundColor DarkGray }
}

# ---------------------------------------------------------------- report
$pass = ($rows | Where-Object Result -eq "PASS").Count
$fail = ($rows | Where-Object Result -eq "FAIL").Count
$skip = ($rows | Where-Object Result -eq "SKIP").Count

$lines = @()
$lines += "# Phase A acceptance - zero provider tokens"
$lines += ""
$lines += ("Run: " + (Get-Date -Format s) + "  |  PASS $pass  FAIL $fail  SKIP $skip")
$lines += ""
$lines += "| # | Result | Evidence |"
$lines += "| --- | --- | --- |"
foreach ($r in ($rows | Sort-Object { [int]$_.Id })) {
  $lines += ("| {0} | {1} | {2} |" -f $r.Id, $r.Result, $r.Evidence)
}
$lines += ""
$lines += "Phases B, C and D remain. See docs/V1_ACCEPTANCE_SESSION.md."
$lines | Set-Content $out -Encoding utf8

Write-Host ""
Write-Host ("PASS $pass   FAIL $fail   SKIP $skip") -ForegroundColor White
Write-Host ("Written: $out") -ForegroundColor DarkGray
if ($fail -gt 0) { Write-Host "Fix the FAILs before spending tokens on Phase B." -ForegroundColor Yellow }
