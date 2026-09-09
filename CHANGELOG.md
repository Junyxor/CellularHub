# Changelog

## 1.0.0

CellularHub 的第一个稳定版候选。1.0 不再扩张功能面，重点转向 Windows 可安装、长期常驻、数据可恢复和问题可诊断。

### Release hardening
- 三个本地数据仓库统一使用临时文件分阶段写入、旧文件 `.bak` 备份和损坏主文件回退读取。
- Provider 连接失败会进入 `CellularSnapshot.lastError`，不会静默失败。
- 新增“1.0 兼容性与诊断”面板：检查平台、数据目录、SMS Provider 和最近错误。
- 可导出隐私安全 JSON 诊断，默认不包含短信正文、验证码、eSIM activation code。
- 诊断记录应用版本、OS/架构、设备数量、短信/未读数量、eSIM 档案数量和当前 Provider 快照。
- Windows CI 扩展到 main / PR / tag，增加前端构建、Rust tests、Tauri bundle 和 SHA256SUMS。
- 版本号统一为 1.0.0，UI 从 PREVIEW 标记切换到 RELEASE。

### Stable scope
- Windows MBN 原生 SMS 优先。
- AT Text -> PDU/TPDU/UDH 作为 SMS receive fallback。
- SMS 发送默认关闭，不作为 1.0 兼容性承诺。
- Windows LPA 为原生 eSIM 主路径；外置真实 eUICC + lpac 仍标记 Experimental。
- Liquid Glass 大窗口、右下角迷你面板、托盘、开机静默启动、会话式短信、eSIM Wallet 档案均保留。
