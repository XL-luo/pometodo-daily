# PomeTodo Daily 分发与对应源码说明

## 修改版身份

PomeTodo Daily 是 `XL-luo` 基于 [ShiliuX-Team/pometodo](https://github.com/ShiliuX-Team/pometodo) 上游提交 `3a74638c273e0cbccc0303f97b1c9e2db7693e3e` 制作的非官方修改版。修改版保留原项目的 Git 历史、GPL-3.0-only 许可和第三方声明，不代表上游作者发布、认可或担保。

## 2026-10-01 已发送的单个 EXE

曾向少量接收者发送以下文件：

- 文件名：`pometodo-desktop.exe`
- 文件版本：`0.1.4`
- 文件大小：`17,208,832` 字节
- 本机生成时间：2026-10-01 16:56:06（Asia/Shanghai）
- SHA256：`5FDF5B9D4BDE1540FF77B8956A6703D114596796AB0F2BA7B85C88FED18DE685`

开发日志和后续基线提交表明，该文件来自 2026-10-01 完成验证的定制工作区，相关应用源码随后保存为提交 `2fadc2d0e14da8ffbc36dbbbd1760bb399656671`。旧二进制没有嵌入 Git 提交号，当时也没有生成独立构建记录，因此不能声称它与该提交存在可独立验证的逐字节构建对应关系。此处保留事实边界，不以更新后的源码冒充旧文件的精确构建来源。

建议接收者下载 [PomeTodo Daily 0.1.4 源码与许可补充发布](https://github.com/XL-luo/pometodo-daily/releases/tag/daily-0.1.4-2026-10-16)。该发布固定到提交 `dbe6ee0c6012832178db51a75dc61383e3ca21cd`，包含安装包、便携程序、许可材料、显式源码归档、构建记录和 SHA256 清单。旧文件仍可用上方 SHA256 识别。

## 获取源码与许可材料

每个正式发布条目固定到一个 Git 标签和提交。发布页自动生成的 `Source code (zip)` / `Source code (tar.gz)` 是该标签的源码快照；发布附件中的 `licenses/` 保存 GPL 正文、修改版说明、第三方声明、Rust 标准库声明、WebView2 声明和所需 MPL 组件源码归档。

构建所需版本记录在 `package.json`、`pnpm-lock.yaml`、`Cargo.toml`、`Cargo.lock`、`src-tauri/Cargo.toml` 和 `src-tauri/Cargo.lock`。Windows 构建环境与命令见[开发指南](DEVELOPMENT.md)。构建依赖可从各自包源取得；发布包中的说明不改变各第三方组件的许可。

## 转发给既有接收者的通知

> 我之前发给你的 PomeTodo Daily 0.1.4 是基于开源项目 PomeTodo 修改的非官方版本。现在已补齐对应源码、GPL 和第三方许可材料，并提供带固定提交和 SHA256 记录的替代版本。请从 https://github.com/XL-luo/pometodo-daily/releases/tag/daily-0.1.4-2026-10-16 下载。你收到的旧 `pometodo-desktop.exe` 如 SHA256 为 `5FDF5B9D4BDE1540FF77B8956A6703D114596796AB0F2BA7B85C88FED18DE685`，就是此次登记的旧文件；由于旧文件没有嵌入提交号，建议改用发布页的新版本。该修改版由 XL-luo 发布，不是 ShiliuX-Team 官方版本。
