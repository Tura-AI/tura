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
$DirectSwitchEvidence = Join-Path $Repo "tests\equivalence\runtime_session\direct-switch-evidence.json"

function Invoke-Capture([string]$Root, [string]$Output) {
  New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
  $temporaryOutput = "$Output.tmp"
  $stderrOutput = Join-Path $OutputDirectory "$([System.IO.Path]::GetFileNameWithoutExtension($Output)).stderr.log"
  Remove-Item -LiteralPath $temporaryOutput, $stderrOutput -Force -ErrorAction SilentlyContinue
  Push-Location $Root
  try {
    & cargo run --quiet --manifest-path (Join-Path $Root "tests\equivalence\runtime_session\runner\Cargo.toml") --bin runtime-session-equivalence > $temporaryOutput 2> $stderrOutput
    if ($LASTEXITCODE -ne 0) {
      Remove-Item -LiteralPath $temporaryOutput -Force -ErrorAction SilentlyContinue
      $stderrTail = (Get-Content -LiteralPath $stderrOutput -Tail 80 -ErrorAction SilentlyContinue) -join "`n"
      throw "equivalence capture failed for $Root; stderr=$stderrOutput`n$stderrTail"
    }
    Move-Item -LiteralPath $temporaryOutput -Destination $Output -Force
    if ((Get-Item -LiteralPath $stderrOutput).Length -eq 0) {
      Remove-Item -LiteralPath $stderrOutput -Force
    }
  } finally {
    Pop-Location
  }
}

function Invoke-Inventory([string]$Root, [string]$Output) {
  New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
  $temporaryOutput = "$Output.tmp"
  $stderrOutput = Join-Path $OutputDirectory "$([System.IO.Path]::GetFileNameWithoutExtension($Output)).stderr.log"
  Remove-Item -LiteralPath $temporaryOutput, $stderrOutput -Force -ErrorAction SilentlyContinue
  Push-Location $Root
  try {
    & cargo run --quiet --manifest-path (Join-Path $Root "tests\equivalence\runtime_session\runner\Cargo.toml") --bin inventory -- $Root > $temporaryOutput 2> $stderrOutput
    if ($LASTEXITCODE -ne 0) {
      Remove-Item -LiteralPath $temporaryOutput -Force -ErrorAction SilentlyContinue
      $stderrTail = (Get-Content -LiteralPath $stderrOutput -Tail 80 -ErrorAction SilentlyContinue) -join "`n"
      throw "phase 0 inventory failed for $Root; stderr=$stderrOutput`n$stderrTail"
    }
    Move-Item -LiteralPath $temporaryOutput -Destination $Output -Force
    if ((Get-Item -LiteralPath $stderrOutput).Length -eq 0) {
      Remove-Item -LiteralPath $stderrOutput -Force
    }
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

function ConvertTo-CanonicalJsonValue([AllowNull()][object]$Value) {
  if ($null -eq $Value) { return $null }
  if ($Value -is [System.Management.Automation.PSCustomObject]) {
    $result = [ordered]@{}
    foreach ($property in ($Value.PSObject.Properties | Sort-Object Name)) {
      $result[$property.Name] = ConvertTo-CanonicalJsonValue $property.Value
    }
    return [pscustomobject]$result
  }
  if ($Value -is [System.Collections.IEnumerable] -and $Value -isnot [string]) {
    $items = [System.Collections.Generic.List[object]]::new()
    foreach ($item in $Value) {
      $items.Add((ConvertTo-CanonicalJsonValue $item))
    }
    return ,$items.ToArray()
  }
  return $Value
}

function ConvertTo-CompactJson([AllowNull()][object]$Value) {
  $canonical = ConvertTo-CanonicalJsonValue $Value
  return ConvertTo-Json -InputObject $canonical -Depth 100 -Compress
}

function Get-JsonPathValue([object]$Root, [object[]]$Path) {
  $cursor = $Root
  foreach ($segment in $Path) {
    $property = $cursor.PSObject.Properties[[string]$segment]
    if ($null -eq $property) {
      throw "JSON path '$($Path -join '.')' is missing segment '$segment'"
    }
    $cursor = $property.Value
  }
  return [pscustomobject]@{ Value = $cursor }
}

function Set-JsonPathValue([object]$Root, [object[]]$Path, [AllowNull()][object]$Value) {
  if ($Path.Count -eq 0) { throw "approved compatibility path must not be empty" }
  $cursor = $Root
  for ($index = 0; $index -lt $Path.Count - 1; $index++) {
    $segment = [string]$Path[$index]
    $property = $cursor.PSObject.Properties[$segment]
    if ($null -eq $property) {
      throw "JSON path '$($Path -join '.')' is missing segment '$segment'"
    }
    $cursor = $property.Value
  }
  $leaf = [string]$Path[$Path.Count - 1]
  $leafProperty = $cursor.PSObject.Properties[$leaf]
  if ($null -eq $leafProperty) {
    throw "JSON path '$($Path -join '.')' is missing leaf '$leaf'"
  }
  $leafProperty.Value = $Value
}

function Assert-FrozenReference([object]$Evidence) {
  $captureHash = (Get-FileHash -Algorithm SHA256 $Reference).Hash
  $inventoryHash = (Get-FileHash -Algorithm SHA256 $Inventory).Hash
  if ($captureHash -cne $Evidence.frozen_phase0.capture_sha256) {
    throw "frozen Phase 0 capture hash changed: expected=$($Evidence.frozen_phase0.capture_sha256) actual=$captureHash"
  }
  if ($inventoryHash -cne $Evidence.frozen_phase0.inventory_sha256) {
    throw "frozen Phase 0 inventory hash changed: expected=$($Evidence.frozen_phase0.inventory_sha256) actual=$inventoryHash"
  }
  Write-Host "frozen Phase 0 capture and inventory hashes match"
}

function Assert-ApprovedCompatibility([string]$Expected, [string]$Actual, [object]$Evidence) {
  $expectedJson = Get-Content -Raw $Expected | ConvertFrom-Json -Depth 100
  $actualJson = Get-Content -Raw $Actual | ConvertFrom-Json -Depth 100
  foreach ($difference in $Evidence.approved_differences) {
    $path = @($difference.path)
    $actualValue = (Get-JsonPathValue $actualJson $path).Value
    $actualValueText = ConvertTo-CompactJson $actualValue
    $approvedValueText = ConvertTo-CompactJson $difference.expected_candidate
    if ($actualValueText -cne $approvedValueText) {
      throw "approved direct-switch value mismatch at '$($path -join '.')': expected=$approvedValueText actual=$actualValueText"
    }
    $referenceValue = (Get-JsonPathValue $expectedJson $path).Value
    Set-JsonPathValue $actualJson $path $referenceValue
  }
  $expectedText = ConvertTo-CompactJson $expectedJson
  $actualText = ConvertTo-CompactJson $actualJson
  if ($expectedText -cne $actualText) {
    throw "runtime/session capture differs outside the approved direct-switch paths"
  }
  Write-Host "Phase 0 behavior matches outside approved direct-switch paths"
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
    $evidence = Get-Content -Raw $DirectSwitchEvidence | ConvertFrom-Json -Depth 100
    Assert-FrozenReference $evidence
    Assert-ApprovedCompatibility $Reference $candidate $evidence
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
