# Quota Float Codex 开发交接（2026-07-28）

## 项目位置

```text
/Users/kwan/Desktop/workspace（atlas）/02 - 项目 (Project)/quota-float
```

技术栈：Tauri 2、Rust、React 19、TypeScript、Vite。

当前分支：`main`。

## 本轮目标

1. 完全移除付费、许可证和支持者解锁模块。
2. Blur 与 Computer 皮肤改为所有用户直接可用。
3. macOS 悬浮窗可以出现在所有显示器和 Space。
4. 切换到另一个 Space 时，收起球和展开卡片自动来到前方。
5. 用户主动点击其他应用后，悬浮窗使用普通窗口层级，可以被覆盖。
6. 用户仍可通过图钉按钮显式开启真正的永久置顶。

## 已完成

### 移除付费模块

- 删除前端支持者购买与许可证激活页面。
- 删除 Rust 许可证验证、设备码和签名逻辑。
- 删除离线许可证 CLI 和维护者签发工具。
- 删除对应付费资源与维护文档。
- 删除许可证相关 Rust 依赖。
- 保留 Blur 与 Computer 外观，并在托盘的“皮肤”菜单中免费开放。

主要提交：

```text
e009a45 feat: remove paid features and support all macOS spaces
```

### macOS 多显示器与 Space 行为

- 窗口启用 `set_visible_on_all_workspaces(true)`。
- 监听 `NSWorkspaceActiveSpaceDidChangeNotification`。
- Space 切换后，在主线程调用 `NSWindow.orderFrontRegardless()`。
- 默认窗口层级改为普通层级，不再默认永久置顶。
- 增加 `windowBehaviorVersion: 1` 偏好版本。
- 旧设置首次读取时会把历史默认 `alwaysOnTop: true` 迁移为 `false`。
- 图钉按钮仍调用原有 `set_widget_always_on_top`，用户可手动开启永久置顶。

核心实现位于：

```text
src-tauri/src/lib.rs
src-tauri/src/models.rs
src-tauri/tauri.conf.json
```

macOS 原生依赖位于：

```text
src-tauri/Cargo.toml
```

## 期望交互

默认状态：

1. Quota Float 同时加入所有 macOS Space。
2. 用户切换 Space 后，Quota Float 自动排到该 Space 的窗口前方。
3. 用户随后点击其他应用窗口时，其他应用可以正常盖住 Quota Float。

图钉开启状态：

1. Quota Float 使用 macOS floating window level。
2. 其他普通窗口不能覆盖它。
3. 再次点击图钉可恢复默认行为。

收起球与展开卡片共用同一个原生 `widget` 窗口，所以两种状态应保持相同行为。

## 已完成验证

```text
npm test
```

结果：5 个测试文件、16 项测试全部通过。

```text
npm run build
```

结果：TypeScript 与 Vite 生产构建通过。

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

结果：Rust 与新增 macOS 原生 API 编译检查通过。

```text
npm run tauri -- build --bundles app
```

结果：release 二进制和 `Quota Float.app` 均成功生成。命令最后会因为没有
`TAURI_SIGNING_PRIVATE_KEY` 而在 updater 签名阶段返回非零；本地 `.app` 不受影响。

## 当前本机安装状态

新版本已经安装并启动：

```text
/Applications/Quota Float.app
```

构建产物：

```text
src-tauri/target/release/bundle/macos/Quota Float.app
```

被替换的旧版本保存在废纸篓：

```text
/Users/kwan/.Trash/Quota Float-old-2026-07-28.app
```

Rust 已通过官方 rustup 安装。新终端若找不到 `cargo`，执行：

```bash
source "$HOME/.cargo/env"
```

## 构建与启动

```bash
cd '/Users/kwan/Desktop/workspace（atlas）/02 - 项目 (Project)/quota-float'
source "$HOME/.cargo/env"
npm ci
npm test
npm run tauri -- build --bundles app
open -n 'src-tauri/target/release/bundle/macos/Quota Float.app'
```

如果旧进程仍在运行，单实例插件可能只会唤醒旧版。先退出：

```bash
osascript -e 'tell application "Quota Float" to quit'
```

## 下一步建议

优先做 macOS 人工交互验收：

1. 图钉关闭，分别测试收起球与展开卡片。
2. 在同一显示器的两个 Space 之间切换，确认每次切换后窗口来到前方。
3. 切换完成后点击 Finder、浏览器或其他应用，确认它们可以覆盖 Quota Float。
4. 在两个物理显示器之间移动窗口并重复上述测试。
5. 开启图钉，确认普通窗口不能覆盖；关闭图钉后恢复默认行为。
6. 重启应用，确认旧偏好只迁移一次，之后手动图钉设置可以正常保存。
7. 测试全屏应用 Space；如窗口需要出现在全屏 Space，可考虑为
   `NSWindowCollectionBehavior` 增加 `FullScreenAuxiliary`，但在确认需求前不要扩展。

## 注意事项

- 不要恢复任何许可证、付费或支持者解锁代码。
- `base64` 仍被 Codex 额度响应解析使用，不能作为许可证残留删除。
- `NSWorkspace` 通知观察者按应用生命周期注册，并故意保留到进程退出。
- 不要把 updater 私钥写入仓库；正式发布时通过安全环境变量提供。
- 当前修改是在提交 `e009a45` 之后继续完成的；使用 `git log -2 --oneline`
  查看最新两个提交。
