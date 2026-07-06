# Install

## npm 安装

```bash
npm install -g tura-ai
tura exec "Hello from Tura"
```

npm 包通过 `npm/tura.mjs` 暴露 `tura` 命令。这个启动器会查找当前平台的 release 包，例如 `tura-win32-x64`、`tura-linux-x64`、`tura-darwin-x64`、`tura-darwin-arm64`，然后把参数转发给真正的 release binary。

常用 npm 启动器命令：

```bash
tura register-cli
tura doctor-cli-path
tura unregister-cli
```

`register-cli` 和 `unregister-cli` 只处理本机 shell/PATH 注册，不会修改项目代码。

## 源码仓库安装

Windows：

```powershell
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
```

macOS/Linux：

```bash
./scripts/install.sh
./scripts/build-release.sh
./scripts/register-cli.sh
```

## install 脚本做什么

安装脚本会准备 Rust、Bun/Node 依赖、GUI/TUI packages、command packages 和 release binary。构建产物通常在 `target/debug` 或 `target/release`。

代码引用：

- `package.json`，npm scripts `install:deps`、`build:release`、`postinstall`。
- `npm/tura.mjs`，函数 `installedReleaseDir`、`platformReleaseDir`。
- `scripts/install.ps1`、`scripts/install.sh`。
- `scripts/build-release.ps1`、`scripts/build-release.sh`。

## 常见问题

| 现象 | 处理 |
| --- | --- |
| `Tura release binary was not found` | 重新安装 `tura-ai`，安装平台包，或运行 `build-release` |
| `tura` 不在 PATH | 运行 `tura register-cli` 或仓库里的 `register-cli` 脚本 |
| Windows 找不到 PowerShell | 只有 PowerShell 在非标准位置时才设置 `TURA_POWERSHELL_PATH` |
| CI 不想改 PATH | 安装前设置 `TURA_NPM_SKIP_CLI_REGISTRATION=1` |
