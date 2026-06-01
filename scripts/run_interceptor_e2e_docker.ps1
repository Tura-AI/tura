# Build the interceptor e2e harness image and run the command interceptor
# end-to-end tests inside a Linux container, where a real `bash` executes the
# commands. The interceptor must block dangerous commands before they run.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$image = "tura-interceptor-e2e"
$dockerDir = Join-Path $root "crates/tools/tests/docker"

docker build -t $image -f (Join-Path $dockerDir "Dockerfile") $dockerDir
if ($LASTEXITCODE -ne 0) { throw "docker build failed" }

docker run --rm `
    -v "${root}:/work" `
    -w /work `
    $image `
    cargo test -p code-tools --test command_interceptor_e2e -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "interceptor e2e tests failed" }
