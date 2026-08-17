# WireGuard 控制台

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

基于 **Tauri v2 + React + TypeScript** 的 WireGuard 桌面管理工具（Linux）。

管理本机的 WireGuard 接口与 Peer：状态监控、Peer 增删改、接口启停、配置文件读写与导入导出。

## 功能

| 模块 | 能力 |
| --- | --- |
| 仪表盘 | 接口状态、地址/MTU/端口、公钥（可脱敏显示）、实时流量与速率、Peer 握手时间（5s 自动刷新） |
| Peer 管理 | 表格内增/删/改，一键生成密钥对与预共享密钥，改动可「仅运行时」或「写入配置并热同步」 |
| 接口控制 | `wg-quick up / down / restart`，`wg syncconf` 热同步（不中断连接） |
| 配置中心 | 读取/编辑 `/etc/wireguard/*.conf`（原始文本），保存、保存并热同步、导入、导出 |

## 安全设计

- **特权桥接**：所有 root 操作经 `pkexec`（polkit）执行，每次操作弹系统授权框；同一会话内 polkit 会记住授权，后续静默放行。
- **密钥不进命令行**：私钥、预共享密钥、`wg setconf` 的配置一律经 **stdin** 传给 `wg`，避免泄露到 `/proc/<pid>/cmdline`。
- **文件名校验**：配置名与接口名均做白名单校验，杜绝路径穿越。
- **文件权限**：写入 `/etc/wireguard/*.conf` 后强制 `chmod 600`。
- 私钥在前端默认脱敏显示，手动点击才展开。

## 架构

```
React UI (src/) ──Tauri IPC──> Rust 后端 (src-tauri/src/)
                                  ├─ wg/elevate.rs   pkexec 桥（密钥走 stdin）
                                  ├─ wg/dump.rs      wg show all dump 解析
                                  ├─ wg/conf.rs      wg-quick 配置解析/序列化
                                  └─ wg/ops.rs       高层操作 + 校验
```

- 接口流量统计读 `/sys/class/net/*/statistics`（免特权）；地址/MTU 经 `ip -j addr`（免特权）。
- Peer/握手等运行态信息经 `pkexec wg show all dump`。

## 安装

从 [Releases](https://github.com/DHKun/Wireguard-GUI/releases) 下载对应包：

```bash
# Debian / Ubuntu
sudo apt install ./wireguard-gui_0.1.0_amd64.deb

# Fedora / RHEL
sudo dnf install ./wireguard-gui-0.1.0-1.x86_64.rpm
```

首次运行需要一次 polkit 授权；如需免密管理（单用户机器），可安装 polkit 规则：

```bash
# /etc/polkit-1/rules.d/50-wireguard-gui.rules
# 允许当前用户在本地活动会话中免密运行 wg / wg-quick / tee / chmod / cat / ls / install
```

## 开发

前置：Rust 工具链、Node ≥ 20、`webkit2gtk-4.1`、`pkexec`（polkit）。

```bash
pnpm install
pnpm tauri dev        # 开发模式（Vite HMR + debug 二进制）
```

## 构建

```bash
pnpm tauri build --no-bundle   # 仅产出 release 二进制
pnpm tauri build               # 打包 deb + rpm（需 dpkg-deb / rpmbuild）
```

release 二进制位于 `src-tauri/target/release/wireguard-gui`，安装包位于
`src-tauri/target/release/bundle/{deb,rpm}/`。

## 测试

```bash
cargo test   # src-tauri 下运行解析器单元测试（dump/conf）
```

## 已知限制

- 序列化配置时会丢弃原文件中的注释（结构保真，注释不保留）。
- 仅支持 Linux（依赖 pkexec 与 /etc/wireguard 约定）。
- 接口关闭时仪表盘不显示该接口（运行态数据来自内核 dump）；Peer 可在配置页查看。
