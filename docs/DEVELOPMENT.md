# 开发指南

本仓库是 React + TypeScript + Tauri 2 的 Windows 客户端，使用 SQLite 保存数据。默认构建只使用自带 API Key，不含官方账户实现。

## 开发环境

- Windows 10 / 11 x64。
- Node.js 22、pnpm 11、Rust stable。
- Visual Studio C++ Build Tools 与 Windows SDK，要求见 [Tauri Windows 环境说明](https://v2.tauri.app/start/prerequisites/#windows)。
- WebView2；截图识别的文字回退路径还需要相应 Windows OCR 语言组件。

```powershell
corepack pnpm install --frozen-lockfile
corepack pnpm tauri dev
```

首次安装或编译需要下载依赖；Rust 的 `--offline` 仅适用于已有完整缓存的情况。调试版使用独立的 `development` 数据目录，验证时请使用虚构资料。

## 从哪里修改

| 想修改的内容 | 入口 |
| --- | --- |
| 待办列表、表单、日历与交互 | `src/features/todo/` |
| 工作／生活／学习的显示文字 | `src/features/todo/wordPacks.ts` |
| 设置页面与 Key | `src/features/settings/` |
| 主窗口、浮球、主题 | `src/app/` |
| 存储、附件与备份恢复 | `crates/todo-core/` |
| Windows 窗口、OCR、AI 请求、提醒与更新 | `src-tauri/src/` |
| 界面与原生能力的数据接口 | `src/contracts/`、`src/native/` |
| 服务扩展接口与默认实现 | `src/extensions/`、`src-tauri/src/service_extension.rs` |

公开构建不需要官方账户源码、服务器或作者密钥。维护者的官网构建通过扩展接口接入单独保存的账户模块；公开目录中没有这部分实现，不要开启需要该私有模块的 `official-services` 功能。

## 检查与构建

按改动范围运行：

```powershell
corepack pnpm test
corepack pnpm run check:host
corepack pnpm run build
cargo test --workspace --locked
cargo test --manifest-path src-tauri/Cargo.toml --lib --tests --locked
```

常规测试使用虚构数据、临时目录及本机模拟服务，不调用付费模型。标为 ignored 的系统 OCR 测试须显式运行；自动测试不代表所有 Windows 配置都已验收。

生成自己的 Windows 安装包：

```powershell
corepack pnpm run build:installer
```

构建脚本要求已跟踪文件处于干净提交，检查版本并生成安装包、便携程序、许可目录与校验记录，默认保存到 `artifacts/byok/`，编译缓存使用独立目录。记录包含准确源码提交和固定提交链接。脚本从本机缓存及 `docs/licenses/` 汇集原始版权许可，并把 MPL 组件源码归档随候选包提供；需要当前 Rust 工具链的 `rust-docs` 组件。本机依赖齐全后可加 `--offline`。构建不会发布或部署。

本日报改版使用独立应用标识 `com.xl.pometodo.daily` 和独立数据目录，不会覆盖原版 PomeTodo。迁移数据时先从原版导出备份，再在日报版导入；旧版任务备份可直接读取。日报版关闭上游自动更新，请从本仓库构建新版本。

OCR 夹具由 `scripts/create-ocr-fixture.ps1` 与 `scripts/create-chat-ocr-fixture.ps1` 使用虚构文字生成。

## 数据与网络

- 任务、附件与备份保存在本机；目前没有跨设备同步，也没有旧 WPF 数据成功转换入口。
- 未启用智能整理时，粘贴保留普通文字及截图附件，不自动填写字段。
- 启用 AI 后，文字发送给所选服务商；截图先尝试发送处理后的图片，失败时用 Windows OCR 提取文字再请求。回退可能产生额外请求，按服务商规则计费。
- API Key 使用 Windows 用户级加密保存，任务 ZIP 备份不包含 Key。
- AI 结果先写入草稿，由用户核对后保存；失败应保留已有输入。
- 启动时检查 ShiliuX 的本渠道更新清单；更新下载校验渠道、网址、大小与 SHA256。本地待办不依赖 GitHub 连通。

## 提交改进

自己电脑上的修改不必提交。希望合入维护版本时，请阅读[参与指南](../.github/CONTRIBUTING.md)，说明改动与验证结果，并确认贡献许可。
