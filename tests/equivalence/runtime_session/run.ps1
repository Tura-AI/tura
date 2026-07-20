param(
  [ValidateSet("capture", "reference", "compare", "self-check", "inventory", "gate")]
  [string]$Mode = "gate",
  [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")),
  [string]$ReferenceRepo,
  [string]$OutputDirectory = (Join-Path $Repo "target\runtime-session-equivalence")
)

$ErrorActionPreference = "Stop"
$RunnerManifest = Join-Path $Repo "tests\equivalence\runtime_session\runner\Cargo.toml"
$Reference = Join-Path $Repo "tests\equivalence\runtime_session\reference\runtime-session.json"
$Inventory = Join-Path $Repo "tests\equivalence\runtime_session\reference\inventory.json"

function Invoke-Capture([string]$Root, [string]$Output) {
  New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
  Push-Location $Root
  try {
    & cargo run --quiet --manifest-path (Join-Path $Root "tests\equivalence\runtime_session\runner\Cargo.toml") --bin runtime-session-equivalence > $Output
    if ($LASTEXITCODE -ne 0) { throw "equivalence capture failed for $Root" }
  } finally {
    Pop-Location
  }
}

function Invoke-Inventory([string]$Root, [string]$Output) {
  New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
  Push-Location $Root
  try {
    & cargo run --quiet --manifest-path (Join-Path $Root "tests\equivalence\runtime_session\runner\Cargo.toml") --bin inventory -- $Root > $Output
    if ($LASTEXITCODE -ne 0) { throw "phase 0 inventory failed for $Root" }
  } finally {
    Pop-Location
  }
}

function Assert-Exact([string]$Expected, [string]$Actual, [string]$Label) {
  $expectedBytes = [System.IO.File]::ReadAllBytes($Expected)
  $actualBytes = [System.IO.File]::ReadAllBytes($Actual)
  $equal = $expectedBytes.Length -eq $actualBytes.Length
  if ($equal) {
    for ($index = 0; $index -lt $expectedBytes.Length; $index++) {
      if ($expectedBytes[$index] -ne $actualBytes[$index]) {
        $equal = $false
        break
      }
    }
  }
  if (-not $equal) {
    $expectedJson = Get-Content -Raw $Expected | ConvertFrom-Json -Depth 100
    $actualJson = Get-Content -Raw $Actual | ConvertFrom-Json -Depth 100
    $expectedText = $expectedJson | ConvertTo-Json -Depth 100 -Compress
    $actualText = $actualJson | ConvertTo-Json -Depth 100 -Compress
    $offset = 0
    while ($offset -lt [Math]::Min($expectedBytes.Length, $actualBytes.Length) -and $expectedBytes[$offset] -eq $actualBytes[$offset]) { $offset++ }
    throw "$Label differs; first raw byte offset=$offset expected_len=$($expectedBytes.Length) actual_len=$($actualBytes.Length); value_equal=$($expectedText -ceq $actualText)"
  }
  Write-Host "$Label exact value/raw-byte match"
}

New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
switch ($Mode) {
  "capture" {
    Invoke-Capture $Repo (Join-Path $OutputDirectory "candidate.json")
  }
  "reference" {
    Invoke-Capture $Repo $Reference
    Invoke-Inventory $Repo $Inventory
  }
  "inventory" {
    Invoke-Inventory $Repo (Join-Path $OutputDirectory "inventory.json")
  }
  "compare" {
    $candidate = Join-Path $OutputDirectory "candidate.json"
    Invoke-Capture $Repo $candidate
    Assert-Exact $Reference $candidate "runtime/session capture"
    if ($ReferenceRepo) {
      $referenceCandidate = Join-Path $OutputDirectory "reference-repo.json"
      Invoke-Capture $ReferenceRepo $referenceCandidate
      Assert-Exact $Reference $referenceCandidate "reference repository capture"
    }
  }
  "self-check" {
    $first = Join-Path $OutputDirectory "self-1.json"
    $second = Join-Path $OutputDirectory "self-2.json"
    Invoke-Capture $Repo $first
    Invoke-Capture $Repo $second
    Assert-Exact $first $second "repeated reference capture"
  }
  "gate" {
    & $PSCommandPath -Mode self-check -Repo $Repo -OutputDirectory $OutputDirectory
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $PSCommandPath -Mode compare -Repo $Repo -ReferenceRepo $ReferenceRepo -OutputDirectory $OutputDirectory
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $PSCommandPath -Mode inventory -Repo $Repo -OutputDirectory $OutputDirectory
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  }
}
