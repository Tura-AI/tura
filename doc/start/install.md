# Install

This page is the GitBook entry for installing Tura. The detailed install and
uninstall reference is maintained in [docs/start/install.md](../../docs/start/install.md).

## Source checkout

```powershell
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace"
```

```sh
git clone https://github.com/Tura-AI/tura.git
cd tura
./scripts/install.sh
./scripts/build-release.sh
./scripts/register-cli.sh
tura exec "Inspect this workspace"
```

`install.*` installs dependencies. `build-release.*` writes release artifacts to
`target/release`. `register-cli.*` adds that directory to PATH.

## NPM release

The npm package entry is [`npm/tura.mjs`](../../npm/tura.mjs). It resolves the
platform release package, sets the runtime root, and forwards to the real Tura
binary.

## Related pages

- [How to start](how-to-start.md)
- [CLI parameters](cli-parameters.md)
- [Scripts](../development/scripts.md)
- [Environment](../development/environment.md)
