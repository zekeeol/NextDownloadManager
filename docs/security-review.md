# 本地源码安全审查

审查对象：本工作区的原项目底层引擎代码及 NDM GPUI 原生桌面（`v0.0.1`）。执行环境为 macOS ARM64。

## 结论与边界

在检查的启动入口、网络请求、浏览器桥接、Tracker 通信、外部进程调用、更新验证和持久化路径中，
**未发现明确的后门、挖矿、隐蔽凭据外传或破坏用户文件的恶意行为证据**。
这是源码审阅与本地功能验证的结论，不是对整个依赖供应链的安全认证。
没有逐个审计所有第三方依赖，也没有实施全量网络抓包、Windows/Linux 动态分析或漏洞扫描。

## 敏感能力及证据

| 能力 | 代码证据 | 判断及使用边界 |
| --- | --- | --- |
| 浏览器数据访问 | `extension/manifest.json`、`extension/background.js`、`crates/nmhost/src/main.rs` | 扩展申请 `<all_urls>`、Cookie、webRequest、scripting、proxy 等广泛权限，用于下载接管。`ndm-nmhost` 仅响应 NDM 专属扩展 ID。 |
| Cookie / 请求头转交 | `crates/core/src/lib.rs::start_file`、`crates/engine/src/download.rs::scoped_headers` | 浏览器提供的 Cookie、Referer 等仅在内存中保存并用于对应下载，不写入 `core.json`；跨源批量链接不携带页面 Cookie。存在同站点/CDN 例外规则，不能理解为绝不向其他主机发送 Cookie。 |
| 外部程序下载和执行 | `crates/tools/src/lib.rs`、`deno.rs`、`ffmpeg.rs`、`proc.rs` | 底层辅助模块按需校验签名/校验和后运行 yt-dlp、Deno、FFmpeg；当前 GPUI 普通下载流程不主动拉起这些媒体工具。 |
| 本地控制接口 | `crates/ipc/src/lib.rs::serve` | 仅绑定 `127.0.0.1` 随机端口，要求随机 token，有连接数、请求大小和时间限制；Unix 数据目录 `0700`、端点文件 `0600`。它不是系统沙箱，同用户身份的恶意进程仍可读取 token。 |
| 远程 / 崩溃模块 | `crates/remote/src/lib.rs`、`crates/crash/src/github.rs` | 保留的独立模块。当前默认 `ndm-backend` / `ndm-app` 未启动 LAN 远程管理服务，也未集成崩溃自动上报窗口。旧版 `egui`（`crates/app`）已完全移除。 |
| Torrent 与 Tracker 通信 | `crates/core/src/torrents.rs`、`crates/torrent`、`patches/librqbit-tracker-comms` | 本地 `.torrent` 预览只读文件不连网；磁力元数据与传输会联系 DHT/Peer/Tracker（`patches/librqbit-tracker-comms` 仅修复磁力元数据阶段 `left` / `EVENT_STARTED` / `numwant` 上报）。Extra Tracker 订阅 URL 在拉取前经校验。新 core 禁用 UPnP 和 LSD。 |

## 本次实现的防护与剩余风险

- UI、服务宿主、core 分离；UI 通过经 token 认证的 IPC 发命令，不在渲染线程直接修改任务或运行下载引擎。
- `DesktopRequest` 为显式枚举，服务端验证命令和参数。任务状态使用私有权限原子写入并保留备份。
- Torrent 文件选择索引和目标路径校验；Unix 使用逐层 `openat` / `NOFOLLOW`，并拒绝硬链接文件。Windows 尚未做动态验证。
- 下载 URL 和本地路径会保存为任务记录，因此 URL 中的访问令牌、签名参数或 userinfo 仍可能落盘。
- 源码依赖仍存在供应链风险；`unsafe_code = "deny"` 在全工作区生效，但未宣称完成完整第三方供应链审计。
- 本地 macOS `.app` 使用 ad hoc 签名（`com.nextdownloadmanager.app`），`TeamIdentifier` 未设置，仅保证本地包完整性。

## 已执行验证

- `cargo check --workspace` 与 `cargo clippy --workspace --all-targets -- -D warnings` 零警告通过。
- `cargo test -p ndm-app`（2 passed）、`cargo test -p ndm-core`（2 unit + 7 workflow tests passed）、`cargo test -p ndm-ipc`（60 passed）、`cargo test -p ndm-crash --test in_frame`（2 passed）。
- `python3 tools/dev/check-layering.py`（确认 `ndm-core`、`ndm-backend` 无 GUI 依赖）。
- `python3 tools/extension/pack.py && node tools/extension/check-pack.mjs && node tools/extension/test.mjs`（116 项扩展检查通过，版本 `0.0.1`）。
- 完整 macOS Release 构建 + `strip -S -x` + `codesign --verify --strict --deep` 通过（整包约 28 MB）。
