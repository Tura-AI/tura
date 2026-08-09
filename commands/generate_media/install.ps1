$ErrorActionPreference = "Stop"
# Thin shim — all logic lives in install.py.
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
python "$repoRoot\scripts\install.py" command generate_media $args
exit $LASTEXITCODE
