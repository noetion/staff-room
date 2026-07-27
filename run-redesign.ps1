<#
.SYNOPSIS
  Runs the Agent Room Liquid Glass redesign end to end, unattended.

.DESCRIPTION
  Executes docs/redesign/steps/step-NN.md one at a time, each in a FRESH
  engine process, so no context carries between steps. After each step the
  driver - not the model - runs the verification gate. A passing step is
  committed. A failing step is handed back to a fresh model with only the
  failing output, up to -MaxRepairs times.

  Between step 14 and step 15 the driver runs a design critique pass over the
  screenshots the QA harness captured, writing docs/redesign/REVIEW.md, which
  step 15 then acts on.

  Nothing in this script prompts for input. It is designed to be started and
  left alone.

.EXAMPLE
  .\run-redesign.ps1
  .\run-redesign.ps1 -DryRun
  .\run-redesign.ps1 -Engine codex -Model gpt-5.6-codex-terra -Effort high
  .\run-redesign.ps1 -From 05 -To 08
  .\run-redesign.ps1 -Only 12 -MaxRepairs 3
  .\run-redesign.ps1 -Resume
#>

[CmdletBinding()]
param(
  [string]$From,
  [string]$To,
  [string]$Only,
  [switch]$Resume,
  [switch]$DryRun,
  [string]$Branch = "redesign/liquid-glass",
  [ValidateSet("claude","codex")]
  [string]$Engine = "claude",
  [string]$Model = "",
  [ValidateSet("", "low", "medium", "high", "xhigh")]
  [string]$Effort = "",
  [int]$MaxRepairs = 2,
  [int]$StepTimeoutMinutes = 45,
  [switch]$ContinueOnFailure,
  [switch]$SkipCritique,
  [string]$CodexArgs = "--skip-git-repo-check",
  [switch]$SkipSmokeTest,
  [switch]$YOLO
)

$ErrorActionPreference = "Stop"

$CodexBaseArgs = @($CodexArgs -split '\s+' | Where-Object { $_ })

$repo      = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
if (-not $repo) { $repo = (Get-Location).Path }
$stepsDir  = Join-Path $repo "docs\redesign\steps"
$rulesFile = Join-Path $repo "docs\redesign\STEP-RULES.md"
$stateFile = Join-Path $repo "docs\redesign\.state.json"
$reportMd  = Join-Path $repo "docs\redesign\REPORT.md"
$logDir    = Join-Path $repo "docs\redesign\logs"
$tmpDir    = Join-Path ([System.IO.Path]::GetTempPath()) "agent-room-redesign"

New-Item -ItemType Directory -Force -Path $logDir, $tmpDir | Out-Null

# --------------------------------------------------------------------- output
function Say  ($m, $c = "Gray")  { Write-Host $m -ForegroundColor $c }
function Head ($m)               { Write-Host ""; Write-Host $m -ForegroundColor White; Write-Host ("-" * 64) -ForegroundColor DarkGray }
function Good ($m)               { Write-Host "  $m" -ForegroundColor Green }
function Bad  ($m)               { Write-Host "  $m" -ForegroundColor Red }
function Warn ($m)               { Write-Host "  $m" -ForegroundColor Yellow }

# ---------------------------------------------------------------- preflight
function Test-Tooling {
  Head "Preflight"
  $missing = @()
  foreach ($t in @("git", "node", "npm", $Engine)) {
    if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { $missing += $t }
  }
  if (@($missing).Count -gt 0) {
    Bad ("missing on PATH: " + ($missing -join ", "))
    Bad "install them and re-run (-Engine selects claude or codex)"
    exit 2
  }
  Good "engine: $Engine"
  $nodeMajor = [int]((node -v).TrimStart("v").Split(".")[0])
  if ($nodeMajor -lt 20) { Bad "Node 20+ required (found $nodeMajor)"; exit 2 }
  Good "git, node $nodeMajor, npm present"
  if (Get-Command cargo -ErrorAction SilentlyContinue) { Good "cargo present (step 15 runs the Rust gate)" }
  else { Warn "cargo not on PATH - step 15 will skip the Rust gate" }

  if (-not (Test-Path (Join-Path $repo "node_modules"))) {
    Say "  installing dependencies..." DarkGray
    if (-not $DryRun) { npm install --prefix $repo 2>&1 | Out-Null }
  }
  Good "dependencies present"

  # Design skills are optional but materially improve steps 04-12. They can be
  # repo-local or installed globally for the engine, so check both - a global
  # Codex skill is loaded in `codex exec` just as it is interactively.
  $home_ = [Environment]::GetFolderPath("UserProfile")
  $skillRoots = @(
    (Join-Path $repo ".codex\skills"),
    (Join-Path $repo ".claude\skills"),
    (Join-Path $repo ".agents\skills"),
    (Join-Path $repo "skills"),
    (Join-Path $home_ ".codex\skills"),
    (Join-Path $home_ ".claude\skills")
  )
  foreach ($s in @("frontend-design", "ui-ux-pro-max-skill")) {
    $found = $skillRoots | Where-Object { Test-Path (Join-Path $_ $s) } | Select-Object -First 1
    if ($found) { Good "skill: $s  ($found)" }
    else { Warn "skill not found on disk: $s  - if your engine loads it from elsewhere, ignore this" }
  }
}

# One live call in exactly the shape the real steps use. If the flags are wrong
# this fails in ten seconds with Codex's own error text, instead of fifteen
# steps later with a misleading one.
function Test-CodexInvocation {
  Head "Codex smoke test"
  $probe = Join-Path $tmpDir "probe.txt"
  # The probe text deliberately contains non-ASCII (an em dash and an accent).
  # The step briefs are full of typographic characters, and a stdin encoding
  # fault only shows up on those - a pure-ASCII probe would pass and every real
  # step would then fail with "input is not valid UTF-8".
  Write-Utf8 $probe "Reply with the single word OK and nothing else $([char]0x2014) no tools, no caf$([char]0x00E9)."
  $out = Join-Path $tmpDir "probe-out.txt"

  $probeArgs = @("exec") + $CodexBaseArgs + @("--sandbox", "read-only", "--model", $CodexModelIds["terra"], "-")
  Say ("  codex " + ($probeArgs -join " ")) DarkGray

  $code = 1
  try {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName  = (Get-Command codex).Source
    $psi.Arguments = ($probeArgs | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '
    $psi.WorkingDirectory       = $repo
    $psi.RedirectStandardInput  = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute        = $false
    $proc = [System.Diagnostics.Process]::Start($psi)
    $pb = [System.IO.File]::ReadAllBytes($probe)
    try {
      $proc.StandardInput.BaseStream.Write($pb, 0, $pb.Length)
      $proc.StandardInput.BaseStream.Flush()
    } catch { }   # engine exited before reading; its stderr explains why
    try { $proc.StandardInput.Close() } catch { }
    $so = $proc.StandardOutput.ReadToEndAsync()
    $se = $proc.StandardError.ReadToEndAsync()
    if (-not $proc.WaitForExit(180000)) { try { $proc.Kill() } catch { }; Bad "smoke test timed out"; exit 2 }
    $code = $proc.ExitCode
    Write-Utf8 $out ($so.Result + "`n" + $se.Result)
  }
  catch {
    Bad "could not start codex: $($_.Exception.Message)"
    exit 2
  }

  if ($code -eq 0) { Good "codex responds to the exec invocation this script uses"; return }

  Bad "codex exited $code on the smoke test. Its output:"
  Get-Content $out -Raw -Encoding UTF8 | ForEach-Object { $_ -split "`n" } |
    Select-Object -Last 30 | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkYellow }
  Bad ""
  Bad "This is a flag or auth problem, not a model-name problem."
  Bad "Check 'codex exec --help', then pass corrected flags, e.g.:"
  Bad "  .\run-redesign.ps1 -Engine codex -CodexArgs '--skip-git-repo-check'"
  Bad "or -SkipSmokeTest to run anyway."
  exit 2
}

function Clear-StaleGitLock {
  $lock = Join-Path $repo ".git\index.lock"
  if (-not (Test-Path $lock)) { return }
  $age = (Get-Date) - (Get-Item $lock).LastWriteTime
  if ($age.TotalMinutes -lt 2) {
    Bad "another git process appears to be running (.git/index.lock is fresh)"
    Bad "close it and re-run, or delete $lock"
    exit 2
  }
  Warn ("removing stale .git/index.lock ({0} min old)" -f [int]$age.TotalMinutes)
  Remove-Item $lock -Force -ErrorAction SilentlyContinue
  if (Test-Path $lock) { Bad "could not remove $lock - delete it and re-run"; exit 2 }
  Good "stale git lock cleared"
}

function Initialize-Branch {
  Head "Branch"
  Clear-StaleGitLock
  Push-Location $repo
  try {
    $current = (git rev-parse --abbrev-ref HEAD).Trim()
    $dirty   = (git status --porcelain)

    if ($current -eq $Branch) {
      Good "already on $Branch"
    }
    else {
      $exists = git rev-parse --verify --quiet "refs/heads/$Branch"
      if ($exists) {
        Say "  switching to existing $Branch" DarkGray
        if (-not $DryRun) { git checkout $Branch | Out-Null }
      }
      else {
        Say "  creating $Branch from $current" DarkGray
        # `checkout -b` carries uncommitted work across, which is what we want:
        # the user's in-flight edits are preserved, then baselined on the new
        # branch so the redesign starts from a clean tree.
        if (-not $DryRun) { git checkout -b $Branch | Out-Null }
      }
      Good "on $Branch"
    }

    if ($dirty -and -not $DryRun) {
      Say "  baselining pre-existing working-tree changes" DarkGray
      git add -A | Out-Null
      git commit -q -m "chore: baseline before Liquid Glass redesign" | Out-Null
      Good "baseline commit created"
    }
    elseif (-not $dirty) {
      Good "working tree clean"
    }
  }
  finally { Pop-Location }
}

# ------------------------------------------------------------------- state
function Read-State {
  $steps = @{}
  $started = (Get-Date).ToString("s")
  if (Test-Path $stateFile) {
    try {
      $json = Get-Content $stateFile -Raw -Encoding UTF8 | ConvertFrom-Json
      if ($json.startedAt) { $started = $json.startedAt }
      if ($json.steps) {
        foreach ($p in $json.steps.PSObject.Properties) { $steps[$p.Name] = $p.Value }
      }
    }
    catch { Warn "could not read $stateFile - starting fresh" }
  }
  return @{ startedAt = $started; steps = $steps }
}
function Write-State ($state) {
  if ($DryRun) { return }
  Write-Utf8 $stateFile ($state | ConvertTo-Json -Depth 8)
}
function Get-StepState ($state, $id) {
  if ($state.steps.ContainsKey($id)) { return $state.steps[$id] }
  return $null
}
function Set-StepState ($state, $id, $value) {
  $state.steps[$id] = $value
}

# ------------------------------------------------------------- step parsing
function Get-Steps {
  Get-ChildItem $stepsDir -Filter "step-*.md" | Sort-Object Name | ForEach-Object {
    $raw = Get-Content $_.FullName -Raw -Encoding UTF8
    $meta = @{ id = ""; title = ""; budget = 40000; gate = @("tsc","test","build"); optional = $false; visual = $false; model = ""; effort = "medium" }
    if ($raw -match '(?s)^---\s*(.*?)\s*---') {
      foreach ($line in ($Matches[1] -split "`n")) {
        if ($line -match '^\s*([a-z]+)\s*:\s*(.+?)\s*$') {
          $k = $Matches[1]; $v = $Matches[2].Trim('"')
          switch ($k) {
            "gate"     { $meta.gate = ($v.Trim('[',']') -split ',' | ForEach-Object { $_.Trim(' ','"') }) }
            "budget"   { $meta.budget = [int]$v }
            "optional" { $meta.optional = ($v -eq "true") }
            "visual"   { $meta.visual = ($v -eq "true") }
            default    { $meta[$k] = $v }
          }
        }
      }
    }
    [pscustomobject]@{
      Id = $meta.id; Title = $meta.title; Budget = $meta.budget
      Gate = $meta.gate; Optional = $meta.optional; Visual = $meta.visual
      Model = $meta.model; Effort = $meta.effort
      Path = $_.FullName
    }
  }
}

# -------------------------------------------------------------------- gates
$GateCommands = @{
  "tsc"         = @{ Label = "type-check";  Cmd = "npx";  Args = @("tsc", "--noEmit") }
  "test"        = @{ Label = "unit tests";  Cmd = "npm";  Args = @("test", "--silent") }
  "build"       = @{ Label = "build";       Cmd = "npm";  Args = @("run", "build") }
  "design-lint" = @{ Label = "design lint"; Cmd = "node"; Args = @("scripts/check-design.mjs") }
  "visual"      = @{ Label = "visual QA";   Cmd = "npm";  Args = @("run", "qa:visual") }
}

function Invoke-Gate ($names, $stepId) {
  $failures = @()
  foreach ($name in $names) {
    if ($name -eq "cargo") {
      if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Warn "cargo not on PATH - skipping the Rust gate (nothing in this redesign touches Rust)"
        continue
      }
      foreach ($c in @(
        @("check", "--manifest-path", "src-tauri/Cargo.toml"),
        @("clippy", "--manifest-path", "src-tauri/Cargo.toml", "--all-targets", "--", "-D", "warnings"),
        @("test", "--manifest-path", "src-tauri/Cargo.toml")
      )) {
        $r = Invoke-Checked "cargo $($c[0])" "cargo" $c
        if (-not $r.Ok) { $failures += $r }
      }
      continue
    }
    if (-not $GateCommands.ContainsKey($name)) { Warn "unknown gate '$name' - skipped"; continue }
    $g = $GateCommands[$name]
    $r = Invoke-Checked $g.Label $g.Cmd $g.Args
    if (-not $r.Ok) { $failures += $r }
  }
  return $failures
}

function Invoke-Checked ($label, $cmd, $argv) {
  Say "    $label..." DarkGray
  $out = Join-Path $tmpDir ("gate-" + [guid]::NewGuid().ToString("N") + ".txt")
  Push-Location $repo
  try {
    & $cmd @argv *> $out
    $code = $LASTEXITCODE
  }
  catch { $code = 1; $_ | Out-File -Append $out }
  finally { Pop-Location }

  $text = if (Test-Path $out) { Get-Content $out -Raw -Encoding UTF8 } else { "" }
  if ($code -eq 0) { Good "$label ok"; return @{ Ok = $true; Label = $label; Output = "" } }

  Bad "$label failed (exit $code)"
  # Keep only the tail - the model does not need 4,000 lines of build noise.
  $tail = ($text -split "`n" | Select-Object -Last 120) -join "`n"
  return @{ Ok = $false; Label = $label; Output = $tail }
}

# ------------------------------------------------------------- model driver

# Escalate the *failed stage*, not the whole run: a first repair deepens
# reasoning on the same model; a second repair moves up a tier, because a
# problem that survives more thinking was probably misunderstood, not
# under-thought.
$EffortLadder = @("low", "medium", "high", "xhigh")
$ModelLadder  = @("luna", "terra", "sol")
# Confirmed against `codex` model list.
$CodexModelIds = @{
  "luna"  = "gpt-5.6-luna"
  "terra" = "gpt-5.6-terra"
  "sol"   = "gpt-5.6-sol"
}

# NOTE: parameters are named $tier* on purpose. PowerShell variable names are
# case-insensitive, so parameters called $model/$effort would shadow the script
# parameters $Model/$Effort inside this function and the ladder would silently
# never escalate.
function Step-Escalate ($tierModel, $tierEffort, $attempt) {
  # An explicit -Model AND -Effort pins every step, including repairs: the user
  # asked for one configuration, so escalating would silently contradict them.
  if ($Model -and $Effort) { return @{ Model = $tierModel; Effort = $Effort } }
  if ($Effort) { $tierEffort = $Effort }
  if ($attempt -le 0) { return @{ Model = $tierModel; Effort = $tierEffort } }

  $ei = $EffortLadder.IndexOf($tierEffort); if ($ei -lt 0) { $ei = 1 }
  $mi = $ModelLadder.IndexOf($tierModel);   if ($mi -lt 0) { $mi = 1 }
  $ei = [Math]::Min($ei + 1, $EffortLadder.Count - 1)

  # First repair deepens reasoning on the same model. A second repair also moves
  # up a tier, because a problem that survived more thinking was probably
  # misunderstood rather than under-thought.
  if ($attempt -ge 2) { $mi = [Math]::Min($mi + 1, $ModelLadder.Count - 1) }
  return @{ Model = $ModelLadder[$mi]; Effort = $EffortLadder[$ei] }
}
function Invoke-Model ($promptPath, $label, $timeoutMinutes, $stepModel, $stepEffort) {
  $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
  $log   = Join-Path $logDir "$label-$stamp.json"

  if ($Engine -eq "codex") {
    # Codex reads the prompt from stdin when given "-". Sandboxed to the repo:
    # it may edit files but cannot reach the network, which is the same posture
    # the step rules ask for.
    $engineArgs = @("exec") + $CodexBaseArgs
    if ($YOLO) { $engineArgs += @("--dangerously-bypass-approvals-and-sandbox") }
    else       { $engineArgs += @("--sandbox", "workspace-write") }

    # -Model on the command line pins every step; otherwise each step uses the
    # tier its frontmatter asks for.
    $useModel = if ($Model) { $Model }
                elseif ($stepModel -and $CodexModelIds.ContainsKey($stepModel)) { $CodexModelIds[$stepModel] }
                else { "" }
    if ($useModel) { $engineArgs += @("--model", $useModel) }
    $useEffort = if ($Effort) { $Effort } else { $stepEffort }
    if ($useEffort) { $engineArgs += @("-c", ('model_reasoning_effort="{0}"' -f $useEffort)) }
    $engineArgs += "-"
    $shown = if ($useModel) { $useModel } else { "default model" }
    Say "    $shown / effort $useEffort" DarkGray
  }
  else {
    $engineArgs = @("-p", "--output-format", "json")
    if ($Model) { $engineArgs += @("--model", $Model) }
    if ($YOLO) {
      $engineArgs += "--dangerously-skip-permissions"
    }
    else {
      # No Bash tool at all: the step rules forbid the model running builds, and
      # withholding the tool enforces that mechanically instead of by request.
      $engineArgs += @("--permission-mode", "acceptEdits",
                       "--allowedTools", "Read,Write,Edit,MultiEdit,Glob,Grep")
    }
  }

  if ($DryRun) {
    Say "    [dry run] $Engine $($engineArgs -join ' ') < $promptPath" DarkGray
    return @{ Ok = $true; Tokens = 0; Cost = 0; Summary = "(dry run)" }
  }

  Say "    invoking model ($label)..." DarkGray
  $started = Get-Date

  Push-Location $repo
  try {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName  = (Get-Command $Engine).Source
    $psi.Arguments = ($engineArgs | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '
    $psi.WorkingDirectory      = $repo
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput= $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute       = $false

    $proc = [System.Diagnostics.Process]::Start($psi)

    # Raw bytes, not Write(string). StandardInput encodes with the console
    # codepage by default, which turns every non-ASCII character in the prompt
    # into bytes the engine rejects as invalid UTF-8.
    $promptBytes = [System.IO.File]::ReadAllBytes($promptPath)
    try {
      $proc.StandardInput.BaseStream.Write($promptBytes, 0, $promptBytes.Length)
      $proc.StandardInput.BaseStream.Flush()
    } catch { }   # engine exited before reading; its stderr explains why
    try { $proc.StandardInput.Close() } catch { }

    $stdout = $proc.StandardOutput.ReadToEndAsync()
    $stderr = $proc.StandardError.ReadToEndAsync()

    if (-not $proc.WaitForExit($timeoutMinutes * 60 * 1000)) {
      try { $proc.Kill() } catch { }
      Bad "model timed out after $timeoutMinutes minutes"
      return @{ Ok = $false; Tokens = 0; Cost = 0; Summary = "timeout" }
    }
    $raw = $stdout.Result
    $err = $stderr.Result
    $exitCode = $proc.ExitCode
  }
  finally { Pop-Location }

  Write-Utf8 $log $raw
  $elapsed = [int]((Get-Date) - $started).TotalSeconds

  $tokens = 0; $cost = 0.0; $summary = ""; $isError = $false

  if ($Engine -eq "codex") {
    if ($err) { $err | Add-Content $log }

    # A non-zero exit means the engine itself failed - bad flags, auth, a
    # rejected prompt. Nothing the gate can diagnose, and repairing it would
    # just repeat it, so say so plainly instead of letting the gate take blame.
    if ($exitCode -ne 0) {
      Bad "codex exited $exitCode without running the step:"
      $detail = @(($err + "`n" + $raw) -split "`n" | Where-Object { $_.Trim() } | Select-Object -First 8)
      $detail | ForEach-Object { Write-Host "      $_" -ForegroundColor DarkYellow }
      return @{ Ok = $false; Tokens = 0; Cost = 0.0; Summary = ($detail -join " "); Seconds = $elapsed; Log = $log }
    }

    # Codex streams human-readable output; keep the tail as the summary and
    # scrape a token count if it reported one.
    $summary = ($raw -split "`n" | Where-Object { $_.Trim() } | Select-Object -Last 14) -join "`n"
    if ($raw -match 'tokens used[:\s]+([\d,]+)') { $tokens = [int](($Matches[1]) -replace ',', '') }
    Say ("    {0}s, {1}" -f $elapsed, $(if ($tokens) { "{0:N0} tokens" -f $tokens } else { "usage not reported" })) DarkGray
    return @{ Ok = $true; Tokens = $tokens; Cost = 0.0; Summary = $summary; Seconds = $elapsed; Log = $log }
  }

  try {
    $json = $raw | ConvertFrom-Json
    $summary = [string]$json.result
    $isError = [bool]$json.is_error
    if ($json.PSObject.Properties.Name -contains "total_cost_usd") { $cost = [double]$json.total_cost_usd }
    if ($json.PSObject.Properties.Name -contains "usage") {
      foreach ($f in @("input_tokens","output_tokens","cache_read_input_tokens","cache_creation_input_tokens")) {
        if ($json.usage.PSObject.Properties.Name -contains $f) { $tokens += [int]$json.usage.$f }
      }
    }
  }
  catch {
    Warn "could not parse model output as JSON (see $log)"
    $summary = ($raw -split "`n" | Select-Object -Last 12) -join "`n"
  }

  if ($err) { $err | Add-Content $log }
  Say ('    {0}s, {1:N0} tokens, ${2:N2}' -f $elapsed, $tokens, $cost) DarkGray

  return @{ Ok = (-not $isError); Tokens = $tokens; Cost = $cost; Summary = $summary; Seconds = $elapsed; Log = $log }
}

function Write-Utf8 ($path, $text) {
  # No BOM: Set-Content -Encoding utf8 writes one on Windows PowerShell 5.1,
  # which would ride along at the head of every prompt and state file.
  [System.IO.File]::WriteAllText($path, $text, (New-Object System.Text.UTF8Encoding $false))
}

function New-Prompt ($parts, $name) {
  $path = Join-Path $tmpDir "$name.md"
  Write-Utf8 $path ($parts -join "`n`n---`n`n")
  return $path
}

# --------------------------------------------------------------- one step
function Invoke-Step ($step, $state) {
  Head "Step $($step.Id) - $($step.Title)"

  $rules = Get-Content $rulesFile -Raw -Encoding UTF8
  $brief = Get-Content $step.Path -Raw -Encoding UTF8
  $prompt = New-Prompt @($rules, $brief) "step-$($step.Id)"

  $totalTokens = 0; $totalCost = 0.0; $attempt = 0; $summary = ""; $sha = ""
  $baseModel = $step.Model; $baseEffort = $step.Effort

  while ($true) {
    $label = if ($attempt -eq 0) { "step-$($step.Id)" } else { "step-$($step.Id)-repair$attempt" }
    $tier = Step-Escalate $baseModel $baseEffort $attempt
    $run = Invoke-Model $prompt $label $StepTimeoutMinutes $tier.Model $tier.Effort
    $totalTokens += $run.Tokens
    $totalCost   += $run.Cost
    if ($run.Summary) { $summary = $run.Summary }

    if ($DryRun) { break }

    if (-not $run.Ok) {
      Bad "the engine did not run the step - retrying would just repeat it"
      Bad "log: $($run.Log)"
      Set-StepState $state $step.Id ([pscustomobject]@{
        status = "engine-error"; attempts = ($attempt + 1); tokens = $totalTokens
        cost = $totalCost; summary = $summary; at = (Get-Date).ToString("s")
      })
      Write-State $state
      return $false
    }

    # package.json may have changed (step 14 only) - reinstall before gating.
    Push-Location $repo
    try {
      $pkgChanged = git diff --name-only HEAD -- package.json
      if ($pkgChanged) { Say "    package.json changed - npm install" DarkGray; npm install 2>&1 | Out-Null }
    }
    finally { Pop-Location }

    Say "  gate: $($step.Gate -join ', ')" DarkGray
    $failures = Invoke-Gate $step.Gate $step.Id

    if (@($failures).Count -eq 0) { break }

    $attempt += 1
    if ($attempt -gt $MaxRepairs) {
      Bad "step $($step.Id) failed after $MaxRepairs repair attempt(s)"
      Set-StepState $state $step.Id ([pscustomobject]@{
        status = "failed"; attempts = $attempt; tokens = $totalTokens
        cost = $totalCost; failures = @($failures | ForEach-Object { $_.Label })
        summary = $summary; at = (Get-Date).ToString("s")
      })
      Write-State $state
      return $false
    }

    $next = Step-Escalate $baseModel $baseEffort $attempt
    $how = if ($Model -and $Effort) { "pinned at" } else { "escalating to" }
    Warn "repair attempt $attempt of $MaxRepairs - $how $($next.Model) / $($next.Effort)"
    $failText = ($failures | ForEach-Object {
      "### Failing: $($_.Label)`n`n``````text`n$($_.Output)`n``````"
    }) -join "`n`n"

    $repairBrief = @"
# Repair pass

Your previous attempt at the step below left the verification gate failing.
Fix **only** what the output names. Do not improve anything else, do not
re-read files you do not need to change, and do not restructure.

$failText
"@
    $prompt = New-Prompt @($rules, $brief, $repairBrief) "step-$($step.Id)-repair$attempt"
  }

  # committed only on a clean gate
  if (-not $DryRun) {
    Push-Location $repo
    try {
      if (git status --porcelain) {
        $before = (git rev-parse HEAD).Trim()
        $addOut = git add -A 2>&1
        # Commit subjects are ASCII on purpose: PowerShell hands native argv to
        # git in the console codepage, and a mangled subject is a poor trade for
        # a typographic dash.
        $msg = "redesign(step-$($step.Id)): " + ($step.Title -replace '[^\x20-\x7E]', '-')
        $commitOut = git commit -q -m $msg 2>&1
        $after = (git rev-parse HEAD).Trim()

        if ($after -eq $before) {
          Bad "git did not create a commit for step $($step.Id):"
          @($addOut) + @($commitOut) | Where-Object { $_ } | Select-Object -First 6 |
            ForEach-Object { Write-Host "      $_" -ForegroundColor DarkYellow }
          Bad "the step's work is on disk but unversioned - fix git, then re-run with -Resume"
          $sha = ""
        }
        else {
          $sha = (git rev-parse --short HEAD).Trim()
          Good "committed $sha"
        }
      }
      else { Warn "no changes to commit" ; $sha = "" }
    }
    finally { Pop-Location }
  }

  $overBudget = $totalTokens -gt $step.Budget
  if ($overBudget) { Warn ("over budget: {0:N0} vs {1:N0}" -f $totalTokens, $step.Budget) }

  Set-StepState $state $step.Id ([pscustomobject]@{
    status = "passed"; attempts = ($attempt + 1); tokens = $totalTokens
    cost = $totalCost; budget = $step.Budget; overBudget = $overBudget
    commit = $sha; summary = $summary; at = (Get-Date).ToString("s")
  })
  Write-State $state
  Good "step $($step.Id) passed"
  return $true
}

# ------------------------------------------------------------ critique pass
function Invoke-Critique ($state) {
  if ($SkipCritique) { Warn "critique skipped"; return }
  Head "Design critique"
  $critique = Get-Content (Join-Path $repo "docs\redesign\CRITIQUE.md") -Raw -Encoding UTF8
  $prompt = New-Prompt @($critique) "critique"

  # The critique needs to look at PNGs, so it gets Read but still no Bash.
  $run = Invoke-Model $prompt "critique" 20 "sol" "medium"
  if (-not $DryRun) {
    Push-Location $repo
    try {
      if (git status --porcelain) {
        git add -A | Out-Null
        git commit -q -m "redesign: design critique" | Out-Null
        Good "REVIEW.md committed"
      }
      else { Warn "critique produced no REVIEW.md - step 15 will proceed without findings" }
    }
    finally { Pop-Location }
  }
  Set-StepState $state "critique" ([pscustomobject]@{
    status = "done"; tokens = $run.Tokens; cost = $run.Cost; at = (Get-Date).ToString("s")
  })
  Write-State $state
}

# ------------------------------------------------------------------ report
function Write-Report ($state, $steps, $elapsed) {
  $lines = @()
  $lines += "# Liquid Glass redesign - run report"
  $lines += ""
  $lines += "Branch: ``$Branch``  |  finished: $(Get-Date -Format s)  |  wall time: $([int]$elapsed.TotalMinutes) min"
  $lines += ""
  $lines += "| Step | Title | Status | Attempts | Tokens | Budget | Commit |"
  $lines += "| --- | --- | --- | --- | --- | --- | --- |"

  $tok = 0; $cost = 0.0
  foreach ($s in $steps) {
    $st = Get-StepState $state $s.Id
    if (-not $st) { $lines += "| $($s.Id) | $($s.Title) | not run | | | $($s.Budget) | |"; continue }
    $tok += [int]$st.tokens
    $cost += [double]$st.cost
    $flag = if ($st.overBudget -eq $true) { " (over)" } else { "" }
    $lines += "| $($s.Id) | $($s.Title) | $($st.status) | $($st.attempts) | $('{0:N0}' -f $st.tokens)$flag | $('{0:N0}' -f $s.Budget) | ``$($st.commit)`` |"
  }
  $lines += ""
  $lines += ('**Total: {0:N0} tokens, ${1:N2}.**' -f $tok, $cost)
  $lines += ""
  $lines += "## Step summaries"
  foreach ($s in $steps) {
    $st = Get-StepState $state $s.Id
    if ($st -and $st.summary) {
      $lines += ""
      $lines += "### Step $($s.Id) - $($s.Title)"
      $lines += ""
      $lines += ($st.summary -split "`n" | ForEach-Object { "> $_" })
    }
  }
  $lines += ""
  $lines += "## Next"
  $lines += ""
  $lines += "- Screenshots: ``docs/redesign/qa/``"
  $lines += "- Machine findings: ``docs/redesign/qa/report.json``"
  $lines += "- Design review: ``docs/redesign/REVIEW.md``"
  $lines += "- Launch and look: ``npm run tauri dev``"

  if (-not $DryRun) { Write-Utf8 $reportMd ($lines -join "`r`n") }
  Head "Report"
  Say ('  {0:N0} tokens, ${1:N2}, {2} min' -f $tok, $cost, [int]$elapsed.TotalMinutes) White
  Say "  $reportMd" DarkGray
}

# -------------------------------------------------------------------- main
$runStarted = Get-Date
Say ""
Say "Agent Room - Liquid Glass redesign" White
Say ("=" * 64) DarkGray
if ($DryRun) { Warn "DRY RUN - nothing will be modified" }

Test-Tooling
Initialize-Branch

$state = Read-State
$steps = Get-Steps
if (@($steps).Count -eq 0) { Bad "no step files found in $stepsDir"; exit 2 }

$selected = @($steps)
if ($Only) { $selected = $steps | Where-Object { $_.Id -eq $Only.PadLeft(2, '0') } }
else {
  if ($From) { $selected = $selected | Where-Object { [int]$_.Id -ge [int]$From } }
  if ($To)   { $selected = $selected | Where-Object { [int]$_.Id -le [int]$To } }
}
if ($Resume) {
  $selected = $selected | Where-Object {
    $st = Get-StepState $state $_.Id
    -not $st -or $st.status -ne "passed"
  }
}

if ($Engine -eq "codex" -and -not $DryRun -and -not $SkipSmokeTest) {
  Test-CodexInvocation
}

Say ""
Say ("Running {0} step(s): {1}" -f @($selected).Count, (($selected | ForEach-Object { $_.Id }) -join ", ")) White

foreach ($step in @($selected)) {
  if ($step.Id -eq "15" -and -not $SkipCritique) {
    $reviewed = Get-StepState $state "critique"
    if (-not $reviewed) { Invoke-Critique $state }
  }

  $passed = Invoke-Step $step $state

  if (-not $passed) {
    if ($step.Optional) { Warn "optional step - continuing"; continue }
    if ($ContinueOnFailure) { Warn "-ContinueOnFailure set - moving on"; continue }
    Bad ""
    Bad "Stopping at step $($step.Id)."
    Bad "Inspect docs/redesign/logs/, fix, then: .\run-redesign.ps1 -Resume"
    Write-Report $state $steps ((Get-Date) - $runStarted)
    exit 1
  }
}

Write-Report $state $steps ((Get-Date) - $runStarted)
Good ""
Good "Redesign run complete on $Branch."
exit 0
