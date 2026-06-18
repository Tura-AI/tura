param(
  [switch]$CheckOnly,
  [switch]$Offline,
  [Alias("h")]
  [switch]$Help
)

$ErrorActionPreference = "Stop"

if ($Help) {
  Write-Host "Usage: commands\image_generate\install.ps1 [-CheckOnly] [-Offline]"
  exit 0
}

Write-Host "image_generate dependencies: ok (Rust-only command)"
