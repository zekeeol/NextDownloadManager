# NDM 原生版浏览器扩展（`v0.0.1`）

扩展与 GPUI 桌面应用使用相同版本（`v0.0.1`，从根 `Cargo.toml` 自动读取）。
它使用独立的 Chromium 公钥和扩展 ID：`mnnniimekbcglecjidnckfjigmmkmgmc`，
原生消息主机名为 `com.nextdownloadmanager.host`。原项目商店扩展不适用于此分支；
本分支尚未在 Chrome / Firefox 商店发布。

## 本地安装

1. 运行 `bash tools/installer/build-local-macos.sh release`（或 `debug`）。把生成的应用放到稳定位置；移动应用后需要重新注册主机路径。
2. 打开应用的 **Settings → General → Register host**，注册当前用户的原生消息主机。
   也可手动运行应用内 `Contents/MacOS/nextdownloadmanager-nmhost --register`。
3. Chromium 浏览器在扩展管理页打开开发者模式，选择“加载已解压的扩展”，加载
   `target/extension/chromium`；应用包内也附有 `Contents/Resources/BrowserExtension/chromium`。
4. Firefox 使用 `about:debugging` 临时加载 `target/extension/firefox/manifest.json`。
   未经 AMO 签名的 XPI 不能当作正式商店扩展永久安装。
5. 使用 NDM 扩展时关闭旧下载器的接管，避免两个扩展同时处理同一个下载。刷新已有网页，在扩展弹窗中确认连接状态。首次注册或主机路径变化后，必要时重启浏览器。

可以用 `--unregister` 撤销主机注册。本次开发验证没有修改用户浏览器的注册或扩展安装。

## 已接通的功能

- HTTP(S)、FTP(S) 直接文件下载、原有类型规则、暂停接管和 Alt 绕过。
- 后台接受任务后才确认交接；拒绝或通信失败时保留浏览器的原始下载。
- 磁力链接点击和右键发送进入原生种子预览；暂停接管时普通磁力点击保留浏览器行为。
- `.torrent` 链接先下载元数据文件，完成后自动打开原生文件选择界面。
- 右键发送链接和最多 100 个链接的批量选择；跨源链接不携带页面的 Cookie 头。
- 弹窗打开应用 / 设置、站点排除与撤销；排除规则保存到私有权限的 `browser-rules.json`。
- 新图标、原生版配色、同版本打包；媒体浮动按钮依据能力协商关闭。
- 同一数据目录只保留一个 GPUI 窗口。宿主通过认证的 IPC 发送命令，UI 轮询界面事件，core 不依赖 GUI。

浏览器的 Cookie、Referer、POST 和代理上下文由主机传递，Cookie / POST 上下文仅在内存中保存。
手动代理和 PAC 由下载引擎处理；需要代理凭据的交接会拒绝，保留浏览器下载。
与现有实现一样，任务 URL 中的令牌仍可能随任务记录落盘。

## 能力边界

原生版尚未接入 yt-dlp / HLS / DASH 视频提取。扩展向原生主机协商 `nativeDesktop`，
关闭质量查询和视频浮动面板，并在原生模式的媒体列表中隐藏流媒体条目；普通直链媒体仍可下载。
超出 100 条或需要分块传送的链接选择会明确拒绝，用户可分成较小选择发送。

Safari 的 WebExtension 源文件会输出到 `target/extension/safari`，需要另行构建并启用
Safari `.appex`；本次没有把生成的文件夹当作已安装可用的 Safari 扩展。
Windows / Linux 和浏览器商店分发未做本机运行验证。原生主机的 `NDM_HEADLESS=1`
可用于独立服务的自动化测试，成功交接后不会主动启动桌面窗口。

## 构建与验证

```sh
python3 tools/extension/pack.py
node tools/extension/check-pack.mjs
node tools/extension/test.mjs
cargo test -p ndm-ipc && cargo test -p ndm-nmhost && cargo test -p ndm-core
```

产物为 `target/extension/ndm-chromium.zip`、`ndm-firefox.xpi` 和供未来商店发布的
`ndm-chrome-store.zip`。后者没有固定 key，不用于当前本地加载流程。

本机验证覆盖：116 项扩展脚本检查、原生主机单元及隔离注册测试、core 下载与浏览器事件测试；
真实 native messaging 长度帧 → IPC → core 的 Cookie 下载及内容校验；浏览器传来的 `.torrent`
在实际 GPUI 窗口自动显示文件列表；打开设置命令；扩展弹窗的本地浏览器预览（模拟浏览器 API）。
没有将实际扩展安装进用户浏览器，也没有把模拟 API 预览声称为浏览器加载扩展的端到端测试。
