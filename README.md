# Mole 桌面版

基于 [Mole CLI](https://github.com/tw93/mole) 的 Mac 清理与维护界面。Tauri 2 + Vue 3 + Rust。

## 设计原则

与 Mole 一脉相承：**先看一眼再删**。

- 扫描、归类、展示在本应用内完成（Rust 只读实现，无破坏性）。
- 清理、卸载、优化等**删除操作一律转发给 `mo` CLI**，由它久经考验的安全层
  （路径校验、应用保护、废纸篓路由、操作日志）执行，GUI 不绕过任何一道检查。
- AI 会话记录（`~/.codex/sessions`、`~/.claude/projects` 等）、模型权重
  （`~/.ollama/models`、`~/.cache/huggingface`）在扫描阶段就被标记为受保护，
  永远不进入清理候选。
- 每一项都标注「为什么不能删」或「删了能不能恢复」，让用户自己判断。

## 功能

| 页面 | 能力 | 实现 |
|---|---|---|
| 状态 | CPU / 内存 / 磁盘 / 网络 / 健康评分，5 秒自动刷新 | Rust 直读（sysinfo） |
| 清理 | 缓存扫描、三档安全分类、执行清理 | Rust 扫描 + `mo clean` 执行 |
| 卸载 | 应用列表、散落数据展示、执行卸载 | Rust 扫描 + `mo uninstall` 执行 |
| 启动项 | 登录项与后台服务管理（BTM 与 Launchd） | Rust 聚合读取 |
| 优化 | DNS刷新、QuickLook重置、LaunchServices重建等系统维护 | Rust 原生无损执行 |
| 分析 | 目录逐层占用、构建产物标记、大文件 | Rust 直读（walkdir） |
| 历史 | 清理会话与删除记录 | Rust 解析 Mole 日志 |

## 构建

前置要求：macOS 12+、Node 20+、pnpm、Rust 1.77+、Xcode Command Line Tools。
执行清理/卸载需要安装 [Mole CLI](https://github.com/tw93/mole)（`brew install mole`）。

```bash
pnpm install          # 安装前端依赖
pnpm app:dev          # 开发模式
pnpm app:build        # 产出 .app 与 .dmg
```

## 发布 (GitHub Release)

项目配置了基于 Git Tag 的 GitHub Actions 自动发布工作流（`.github/workflows/release.yml`）：

```bash
# 1. 本地打上版本 tag（以 v 开头）
git tag v0.1.0

# 2. 推送 tag 到 GitHub 触发自动发布
git push origin v0.1.0
```

GitHub Actions 将自动执行以下流程：
- 在 macOS 环境下配置前端与 Rust 工具链
- 编译生成 **Universal 通用架构 DMG**（单包同时原生兼容 Apple Silicon M系列与 Intel Mac）
- 自动根据提交历史生成 Release Notes 并发布到 GitHub Releases

## 结构

```
src/                  Vue 3 前端
src/types.ts          与 Rust 结构对应的类型
src-tauri/src/
  safety.rs           路径保护判断（复刻 Mole 的只读规则）
  scan.rs             磁盘扫描与清理候选归类
  status.rs           系统状态采集与健康评分
  apps.rs             已安装应用扫描
  startup.rs          启动项与后台服务管理
  optimize.rs         系统维护与优化任务
  history.rs          Mole 操作日志解析
  cli.rs              Mole CLI 定位与子进程执行
  commands.rs         Tauri 命令层
```

## 测试

```bash
cd src-tauri
cargo test            # 81 个单元测试
cargo check           # 类型检查
```

## 许可

GPL v3，与 Mole CLI 一致。
