# CellularHub 1.0 build notes

## 1.0 release gate

Windows Actions 必须依次通过：

1. `npm install`
2. `npm run build`（TypeScript + Vite）
3. `cargo test --manifest-path src-tauri/Cargo.toml`
4. `npm run tauri build`
5. 生成 MSI / NSIS EXE / `SHA256SUMS.txt`

当前生成环境没有 Rust 工具链，且 npm registry 在本环境中持续超时，因此这里不能声称 Windows MBN COM / WinRT Toast 已完成真实 cargo 编译。`windows-latest` Actions 是 1.0 的真实发布门槛。

## Data safety

`messages.json`、`conversation-meta.json`、`esim-profiles.json` 使用统一持久化策略：

- 写入同目录 `.tmp`
- flush + `sync_all`
- 旧主文件复制为 `.bak`
- 替换主文件
- 主文件解析失败时尝试 `.bak`

## Diagnostics privacy

导出的 `CellularHub-diagnostics-*.json` 不包含短信正文、验证码或 eSIM activation code。它只包含版本、平台、架构、设备/能力计数、Provider 快照、错误状态和本地数据计数。
