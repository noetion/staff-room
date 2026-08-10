[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $scriptRoot "..")).Path
$unitSeparator = [char]0x1f

if (-not [string]::IsNullOrWhiteSpace($env:RUSTFLAGS)) {
  throw "Release builds require RUSTFLAGS to be unset so the audited path-remapping flags cannot be overridden."
}

if (-not [string]::IsNullOrWhiteSpace($env:CARGO_ENCODED_RUSTFLAGS)) {
  throw "Release builds require CARGO_ENCODED_RUSTFLAGS to be unset so the audited path-remapping flags cannot be overridden."
}

$privatePrefixes = [System.Collections.Generic.List[object]]::new()
$seenPrefixes = [System.Collections.Generic.HashSet[string]]::new(
  [System.StringComparer]::OrdinalIgnoreCase
)

function Add-PrivatePrefix {
  param(
    [string]$Path,
    [string]$Replacement
  )

  if ([string]::IsNullOrWhiteSpace($Path)) {
    return
  }

  $fullPath = [System.IO.Path]::GetFullPath($Path).TrimEnd("\", "/")
  if ($seenPrefixes.Add($fullPath)) {
    $privatePrefixes.Add([pscustomobject]@{
      Path = $fullPath
      Replacement = $Replacement
    })
  }
}

Add-PrivatePrefix -Path $repositoryRoot -Replacement "staff-room"
Add-PrivatePrefix -Path ([Environment]::GetFolderPath("UserProfile")) -Replacement "user-home"
Add-PrivatePrefix -Path $env:CARGO_HOME -Replacement "cargo-home"
Add-PrivatePrefix -Path $env:RUSTUP_HOME -Replacement "rustup-home"

$privatePrefixes = @($privatePrefixes | Sort-Object { $_.Path.Length } -Descending)
$releaseFlags = [System.Collections.Generic.List[string]]::new()
foreach ($prefix in $privatePrefixes) {
  $releaseFlags.Add("--remap-path-prefix=$($prefix.Path)=$($prefix.Replacement)")
}
$releaseFlags.Add("-C")
$releaseFlags.Add("strip=symbols")

$tauriConfigPath = Join-Path $repositoryRoot "src-tauri\tauri.conf.json"
$tauriConfig = Get-Content -LiteralPath $tauriConfigPath -Raw | ConvertFrom-Json
$binaryPath = Join-Path $repositoryRoot "src-tauri\target\release\staff-room.exe"
$installerName = "{0}_{1}_x64-setup.exe" -f $tauriConfig.productName, $tauriConfig.version
$installerPath = Join-Path $repositoryRoot "src-tauri\target\release\bundle\nsis\$installerName"

try {
  $env:CARGO_ENCODED_RUSTFLAGS = $releaseFlags -join $unitSeparator
  Push-Location -LiteralPath $repositoryRoot
  try {
    & npm run tauri build
    if ($LASTEXITCODE -ne 0) {
      throw "Tauri release build failed with exit code $LASTEXITCODE."
    }
  }
  finally {
    Pop-Location
  }

  if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Release binary was not produced at $binaryPath."
  }

  if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
    throw "NSIS installer was not produced at $installerPath."
  }

  $binaryBytes = [System.IO.File]::ReadAllBytes($binaryPath)
  $asciiBinary = [System.Text.Encoding]::ASCII.GetString($binaryBytes)
  $utf16Binary = [System.Text.Encoding]::Unicode.GetString($binaryBytes)

  foreach ($prefix in $privatePrefixes) {
    $asciiMatch = $asciiBinary.IndexOf(
      $prefix.Path,
      [System.StringComparison]::OrdinalIgnoreCase
    )
    $utf16Match = $utf16Binary.IndexOf(
      $prefix.Path,
      [System.StringComparison]::OrdinalIgnoreCase
    )

    if ($asciiMatch -ge 0 -or $utf16Match -ge 0) {
      throw "Release binary contains an unremapped builder-local path. Distribution is blocked."
    }
  }

  $installer = Get-Item -LiteralPath $installerPath
  $installerHash = Get-FileHash -LiteralPath $installerPath -Algorithm SHA256
  $relativeInstallerPath = $installer.FullName.Substring($repositoryRoot.Length).TrimStart("\", "/")
  Write-Output "Release binary path scan passed."
  Write-Output ("Installer: {0}" -f $relativeInstallerPath)
  Write-Output ("Bytes: {0}" -f $installer.Length)
  Write-Output ("SHA-256: {0}" -f $installerHash.Hash)
}
finally {
  Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
}
