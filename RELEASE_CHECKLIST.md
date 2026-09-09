# CellularHub 1.0 release checklist

`1.0.0` 源码版本已经冻结功能面。正式打 `v1.0.0` tag 前必须满足：

- [ ] Windows Actions: frontend build green
- [ ] Windows Actions: Rust tests green
- [ ] Windows Actions: Tauri MSI + NSIS build green
- [ ] `SHA256SUMS.txt` 生成并随 Artifact/Release 发布
- [ ] Windows 11 Home 实机：应用可启动、托盘/迷你窗正常
- [ ] Windows 11 Pro 实机：应用可启动、托盘/迷你窗正常
- [ ] 至少一台 Windows MBN/MBIM 设备：真实 SMS receive 验收
- [ ] 至少一台 AT Modem：Text SMS 或 PDU fallback 真实验收
- [ ] 中文 UCS-2、GSM 7-bit、长短信 UDH 至少各验证一次
- [ ] Windows 11 24H2+：`lpa:` 原生 eSIM handoff 验收
- [ ] 无真实 eUICC 的设备不会显示“可兼容安装 eSIM”
- [ ] `messages.json` / `conversation-meta.json` / `esim-profiles.json` 重启后正常恢复
- [ ] 人为损坏主 JSON 后 `.bak` 回退测试通过
- [ ] 诊断 JSON 不包含短信正文、验证码或 activation code
- [ ] 关闭到托盘、开机静默启动、Single Instance 验收
- [ ] 发短信仍保持关闭，UI 不误报能力

如果 Windows CI 未通过，版本号可以保持 `1.0.0`，但不要发布正式 Release。
