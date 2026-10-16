# PomeTodo Daily

这是由 `XL-luo` 基于 [ShiliuX-Team/pometodo](https://github.com/ShiliuX-Team/pometodo) 制作的非官方修改版。它保留原项目的 GPL-3.0-only 许可和版权历史，不代表 ShiliuX-Team 官方发布或担保。

Windows 上的轻量待办与日报工具：记录任务、日期、备注和截图，并从待办状态与截止日期生成每日工作总结和明日计划。

公开源码方便你按自己的习惯修改界面、添加功能，做一款适合自己的待办工具。

[本修改版发布页](https://github.com/XL-luo/pometodo-daily/releases) · [上游源码](https://github.com/ShiliuX-Team/pometodo) · [开发指南](docs/DEVELOPMENT.md) · [分发与源码说明](docs/DISTRIBUTION.md)

## 能做什么

- 本地记录、搜索和归档待办，保存截图附件，导出与恢复备份。
- 用文字或截图进行 AI 整理，核对草稿后再保存。
- 日报自动汇集当天完成、进行中、当天新增、已到期和明日截止的待办，也可补充临时工作与随手记。
- 支持工作、生活、学习场景，以及浮球快捷入口。

**本仓库的软件免登录，没有官方账户模块。** 本地待办可直接使用；AI 整理填写自己的 API Key，支持 DeepSeek、千问和智谱。需要官方账户与 AI 服务，可使用 ShiliuX 官网提供的安装包；官网版也支持自带 Key。

## 下载或修改

只想使用软件，请在本修改版发布页查找 Windows 安装包或便携程序，并按发布页的 SHA256 校验文件。GitHub 的“Download ZIP”下载的是源码，解压后不能直接当作软件运行。曾收到 2026-10-01 单个 EXE 的用户请先阅读[分发与源码说明](docs/DISTRIBUTION.md)。

想修改源码，先准备 Node.js 22、pnpm 11、Rust 和 Windows 构建工具，具体见[开发环境](docs/DEVELOPMENT.md#开发环境)，然后在项目根目录运行：

```powershell
corepack pnpm install --frozen-lockfile
corepack pnpm tauri dev
```

调试版使用独立数据目录。常用修改入口与检查命令见[开发指南](docs/DEVELOPMENT.md)。

## 数据与范围

任务、日报和附件保存在本机。使用 AI 时，所选待办和补充文字会发送给你选择的服务商，费用由自己的服务商账户承担；截图整理仍沿用待办录入流程。此日报改版不连接上游自动更新。详情见[数据与网络](docs/DEVELOPMENT.md#数据与网络)。

当前公开源码不含官方账户实现、网关后台、服务密钥或真实用户资料。公开前已检查全部 Git 历史、分支、标签和远程发布内容；检查不能替代接收者对下载内容的独立核对。早期公开版本及其许可保留在历史记录中。手机版与跨设备同步尚未交付。

## 许可与参与

采用 [GPL-3.0-only](LICENSE)，允许使用、修改和商业分发；分发受 GPL 覆盖的程序时，须按许可证提供对应源码并保留必要声明。第三方许可和品牌范围见[版权说明](docs/NOTICE.md)。

本修改版由 `XL-luo` 发布，修改内容仍按 GPL-3.0-only 提供；没有向上游额外授予 MIT 许可。上游贡献规则只在向上游提交改进时适用。安全问题请勿附带真实待办、截图或密钥公开提交。
