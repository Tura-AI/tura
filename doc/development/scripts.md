# Scripts

仓库根目录的 `scripts/` 是开发、构建、注册 CLI、启动和 CI 的入口。

## 常用脚本

| Windows | macOS/Linux | 用途 |
| --- | --- | --- |
| `scripts/install.ps1` | `scripts/install.sh` | 安装依赖 |
| `scripts/build-debug.ps1` | `scripts/build-debug.sh` | debug build |
| `scripts/build-release.ps1` | `scripts/build-release.sh` | release build |
| `scripts/register-cli.ps1` | `scripts/register-cli.sh` | 注册 CLI 到 PATH/profile |
| `scripts/unregister-cli.ps1` | `scripts/unregister-cli.sh` | 移除 CLI 注册 |
| `scripts/start.ps1` | `scripts/start.sh` | 启动本地服务/GUI |
| `scripts/run-ci.ps1` | `scripts/run-ci.sh` | CI 检查 |
| `scripts/check-backend-quality.ps1` | `scripts/check-backend-quality.sh` | 后端质量检查 |

## npm scripts

`package.json` 里定义了 release 和 CI 入口：

```bash
npm run install:deps
npm run build:release
npm run package:release
npm run ci
```

代码引用：`package.json` 的 `scripts`。

## release/npm scripts

`scripts/npm/` 处理 npm release 包、platform 包、安装 release archive 和验证安装。

关键文件：

- `scripts/npm/install-release.mjs`
- `scripts/npm/package-release.mjs`
- `scripts/npm/package-platform.mjs`
- `scripts/npm/verify-platform-install.mjs`
- `scripts/npm/cli-path.mjs`

## 脚本原则

- 构建产物进 `target/debug` 或 `target/release`。
- 注册 CLI 和构建分开。
- release 包不要假装 npm uninstall lifecycle 能可靠清理 PATH；用 `unregister-cli`。
