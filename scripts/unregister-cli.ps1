$ErrorActionPreference = "Stop"
# Thin shim — all logic lives in install.py.
python "$PSScriptRoot\install.py" unregister-cli $args
exit $LASTEXITCODE
