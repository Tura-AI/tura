param([string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")))

$ErrorActionPreference = "Stop"
$manifestPath = Join-Path $PSScriptRoot "manifest.json"
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json

Push-Location $Repo
try {
  foreach ($replacement in $manifest.replacements) {
    Write-Host "==> $($replacement.boundary)"
    & cmd.exe /d /s /c $replacement.command
    if ($LASTEXITCODE -ne 0) {
      throw "replacement failed for $($replacement.removed): $($replacement.command)"
    }
  }
} finally {
  Pop-Location
}

Write-Host "behavior-test replacement manifest passed"
