$ErrorActionPreference = "Stop"
# Thin shim — all logic lives in install.py.
python "$PSScriptRoot\install.py" register-cli $args
exit $LASTEXITCODE
