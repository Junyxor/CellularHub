# CellularHub

> 1.0 稳定版目标：Windows 原生蜂窝优先，兼容层可靠收短信，eSIM 不伪造硬件能力，并具备可恢复的数据层与可导出的隐私安全诊断。

Windows 11 蜂窝网络 / eSIM / SMS 统一控制中心。核心原则：**原生能力优先；系统没有可用 SMS 接口时，用兼容层尽可能把短信接出来；eSIM 兼容只建立在真实 eUICC 上，不伪造不存在的硬件。**

## 1.0 当前实现

### 双窗口形态与视觉系统

- **完整模式**：1240×790 控制中心，概览 / 短信 / eSIM / 设备完整管理。
- **迷你模式**：386×520，自动移动到当前显示器右下角、置顶、无侧栏，是独立布局而不是把大窗口缩小。
- Windows 11 上尝试使用系统 **Mica** 材质；React 内容层升级为可调 **Liquid Glass**：多层高光、环境染色、轻量折射、不同尺寸的材质厚度和连续圆角。
- 新增“设置与体验”页面；外观部分：玻璃清晰度、染色强度、动态折射、界面动效、语义彩色文字与 4 组主强调色可即时调节。
- 外观设置保存在 `localStorage`，完整模式与右下角迷你模式自动共享。
- 语义色只用于网络制式、信号、SMS、eSIM、费用与续费/到期等状态；正文仍保持灰白主层级，且可一键关闭彩色文字。
- eSIM 资产以 Wallet 卡片展示：SVG 国旗、使用状态、运营商、号码、套餐、保号费、续费/到期和标签一眼可见。
- 迷你模式直接显示当前线路、环形信号、最近短信/验证码和最近续费提醒。
- 两种模式共用同一进程、短信状态和 eSIM 档案。


### 后台常驻、托盘与开机启动

- 新增原生 Tauri 系统托盘；应用关闭按钮默认隐藏到托盘而不是结束进程。
- 托盘左键：窗口隐藏/最小化时直接唤出右下角迷你面板；窗口已经可见时仅聚焦。
- 托盘右键菜单：完整窗口、迷你面板、短信、eSIM 管理、退出。
- 新增“关闭后驻留托盘”开关；关闭该开关后关闭窗口会真正退出。
- 新增官方 autostart 集成；启用后 Windows 登录时以 `--background` 静默启动并驻留托盘，不主动弹大窗口。
- 新短信到达时，仅在窗口原本隐藏/最小化且用户允许时唤出迷你面板；正在使用完整窗口时不会强制切换。
- 启动时发现 7 天内 eSIM 续费/到期提醒，可在后台状态下唤出迷你面板。
- 启动时可自动连接可用的 Windows 原生 MBN SMS Provider；不会自动抢占 AT 串口。
- 新增 Single Instance：已经运行时再次启动只聚焦现有实例，避免两个进程同时争抢 WWAN/COM 设备。

### 会话资料、置顶与通知路由

- 每个发送号码都可以保存独立别名、备注和置顶状态，持久化到 `conversation-meta.json`。
- 会话搜索同时覆盖别名与备注；置顶会话始终排在普通会话之前，同组内仍按最新短信排序。
- Windows 安装版会优先尝试 WinRT Toast 增强通知；点击通知正文或“打开会话”会把现有托盘进程展开到对应短信会话。
- 验证码通知会尝试提供“复制验证码”原生动作，并通过 Rust 剪贴板直接写入验证码；如果系统不接受自定义 Toast，则自动回退到稳定的普通通知。
- 右下角迷你面板的验证码胶囊也可直接复制，因此原生 Toast 动作不可用时仍有可靠的一键复制入口。
- v1.0 只保证运行中的托盘进程处理 Toast 激活，不宣称已经实现应用完全退出后的冷启动 COM 激活。

### 短信会话中心与本地智能识别

- 短信页默认升级为会话模式：同一发送号码自动聚合，左侧显示最近会话，右侧显示完整时间线；仍可一键切回“原始短信”列表排查底层数据。
- 会话会显示未读数量、总短信数、最近时间和验证码数量；点击会话会一次性把该发送方当前收件箱短信标为已读。
- 会话级归档/恢复由 Rust `CellularManager` 批量执行并立即持久化，不依赖前端逐条模拟。
- 对 `【服务名】` / `[服务名]` 形式的短信签名进行本地提取：会话标题优先显示服务名，同时保留真实发送号码。
- 本地规则把短信分成验证码、账单/续费、流量/套餐和普通通知四类，用低饱和语义色提示；不上传短信正文，也不依赖云端 AI。
- 验证码只在检测到验证码/OTP/安全码等语义关键词时提取 4–8 位数字，降低把手机号、余额或日期误判成验证码的概率。
- 会话时间线中的验证码会提升成独立复制卡片；原始短信模式同样保留验证码一键复制。

### 日常使用与数据可靠性

- 短信历史从内存升级为应用数据目录持久化 `messages.json`，最多保留 1000 条；完全退出后重新打开仍可查看。
- 短信支持搜索、收件箱/归档切换、单条归档/恢复和“全部已读”；归档短信自动取消未读状态。
- 托盘 tooltip 会显示未读数量，图标右下角显示 1–9 / 9+ 徽标；清空未读后恢复默认图标。
- 记住默认/上次使用设备；启动时若它是可收短信的 Windows MBN Provider，会优先恢复。AT 设备只记住，不自动抢占 COM 口。
- eSIM 资产新增关键词搜索、状态筛选，并支持按最近期限、最近更新、保号成本或名称排序。
- eSIM 到期/续费通知按本地日期去重，每天最多主动检查一次；常驻运行时每 30 分钟确认是否跨日。
- 自动唤出的右下角迷你提醒默认 12 秒后收回托盘；手动打开的迷你模式不会被自动隐藏。
- 完整模式和迷你模式加入更连贯的 Liquid Glass 进入过渡，并遵循“界面动效”开关。

### 原生 Windows SMS

Windows Mobile Broadband COM Provider：

- `IMbnInterfaceManager` 枚举 Windows MBN/MBIM 蜂窝接口；
- 读取设备型号、运营商与信号；
- 只在设备明确暴露 `MBN_SMS_CAPS_PDU_RECEIVE` 时标记“原生可收短信”；
- `IMbnSms::SmsRead` 拉取新 PDU；
- `IMbnSmsEvents` + COM Connection Point 接收 SMS 状态/新消息事件；
- 复用现有 PDU/UDH 解码、长短信重组、Windows 原生通知和验证码识别。

当前 **不对外开放发送短信**。`smsSend` 对 Windows MBN 和 AT Provider 均保持 `false`，先保证接收链路可靠。

### SMS 兼容层

当 Windows 原生 MBN SMS 不可用时，可选择 AT 后端：

- Windows COM/USB 串口枚举；
- `AT+CMGF=1` 文本模式，失败自动回退 `AT+CMGF=0` PDU；
- `AT+CNMI` / `+CMTI` / `CMGR` / `CMGL`；
- GSM 7-bit / UCS-2 / 8-bit；
- alphanumeric sender；
- 8-bit / 16-bit UDH multipart 重组；
- `AT+COPS?` 运营商与 `AT+CSQ` 信号；
- Windows 原生通知和验证码一键复制。

因此目标路径是：

```text
Windows MBN / IMbnSms       <- 原生优先
          ↓ 不可用
AT Text SMS
          ↓ 不可用
AT PDU / TPDU + UDH         <- 兼容兜底
```

### eSIM：原生 + 实验兼容

**原生路径**：

- 接收 `LPA:` URI、`1$...` activation code、剪贴板或二维码图片；
- Windows 11 24H2+ 下交给系统 `lpa:` handler / Windows LPA 完成安装；
- 激活信息默认不写入长期档案。

**实验兼容路径**：

- 面向“Windows 自己没有可用 eSIM UI/API，但外置 Modem/读卡设备存在真实 eUICC”的情况；
- 对 AT 端口做只读探测：`AT+CCHO=?`、`AT+CSIM=?`、Quectel `AT+QESIM=?`；
- 能读取时识别 32 位 EID；
- 可选寻找同目录、`tools/` 或 `PATH` 中的 `lpac.exe`；
- 只有 `lpac + CCHO/CGLA 或 CSIM APDU` 同时成立时才开放“实验安装”；
- 安装前会断开占用同一串口的 SMS Provider，避免 COM 口争用；
- **完全没有 eUICC 时不会显示成可安装 eSIM。**

lpac 的 AT APDU backend 本身属于实验/演示性质，部分 eUICC 操作可能受 Modem AT 响应时限影响，因此该路径始终标记为 Experimental。

### eSIM 资产管理

每张 eSIM 可保存：名称、ISO 国家/地区代码与 SVG 国旗、运营商、号码、ICCID、套餐、保号/续费金额、币种、周期、续费日、到期日、自动续费状态、备注、标签和任意自定义字段。

档案保存在应用数据目录 `esim-profiles.json`。应用启动时会检查未来 7 天的续费/到期项目并发送 Windows 通知。

## Windows CI 构建

`.github/workflows/build-windows.yml` 会在 `windows-latest` 上执行前端 TypeScript/Vite 构建、Rust tests 和 Tauri bundle，并生成 MSI、NSIS EXE 与 `SHA256SUMS.txt`。main/PR/手动运行都会作为编译门槛；打 `v*` tag 时同时发布到 GitHub Release。

## 本地开发（Windows）

需要 Node.js 22+、Rust stable、Visual Studio C++ Build Tools 和 WebView2。

```powershell
npm install
npm run tauri dev
```

只看 UI：

```powershell
npm install
npm run dev
```

浏览器模式会使用 Demo Provider，不会执行真实 Modem/eSIM 操作。

## 可选 lpac

CellularHub **不在源码包里捆绑 lpac 二进制**。如需实验 eUICC Bridge，可把官方 `lpac.exe` 放在：

```text
CellularHub.exe
lpac.exe
```

或：

```text
CellularHub.exe
tools/lpac.exe
```

也可以加入 `PATH`。见 `tools/README.md`。

## 实机测试顺序

1. 先到“设备”页尝试 `Windows Native` MBN 设备。
2. 如果 MBN 没暴露 PDU SMS，再选择对应 AT COM 口作为 `SMS FALLBACK`。
3. 给 SIM/eSIM 发一条真实短信，验证 PDU/中文/长短信/通知。
4. eSIM 安装优先使用 Windows Native LPA。
5. 只有 Windows LPA 不可用且硬件存在真实 eUICC 时，再扫描 Experimental eUICC Bridge。

## 架构

```text
React UI
   │ Tauri invoke / events
   ▼
Cellular Core (Rust)
   ├── CellularManager + SMS Store
   ├── eSIM Profile Store
   ├── WindowsMbnProvider       [原生 SMS]
   ├── AtProvider               [SMS 兼容]
   ├── PDU / TPDU / UDH         [共享解码]
   ├── Windows LPA              [原生 eSIM]
   └── eSIM Compat / lpac       [真实外置 eUICC，实验]
```

## 当前边界

- 必须存在能实际接收 SMS 的蜂窝 Modem/SIM 通道；软件不能凭空生成无线基带。
- 没有真实 eUICC 时无法靠兼容层生成 eSIM。
- SMS 发送暂不开放；先完成多模组实机验证后再决定是否加入。
- Windows 原生 MBN 当前只启用 PDU receive；CDMA text-only MBN 设备不会被误标为已支持。
- eSIM 兼容安装依赖可选 lpac 和设备 APDU 能力，属于实验功能。


## v1.0 highlights

- Persistent `conversation-meta.json` for sender aliases, notes, and pin state.
- Pinned conversations sort ahead of normal threads while preserving recency inside each group.
- Search covers aliases and notes in addition to sender/body.
- Windows native toast path can route activation back to a specific conversation while the tray process is alive.
- OTP toasts attempt a native Copy action on Windows; the quick panel always provides a copy chip as the reliable fallback.
- Native toast creation is best-effort and falls back to `tauri-plugin-notification` when AppUserModelID/packaging conditions are not available.
- Sending SMS remains disabled.

## 1.0 stable milestone

CellularHub 1.0 将功能边界固定为“原生完整、兼容层优先可靠接收短信”。Windows MBN 是首选 SMS Provider；AT Text 和 PDU/TPDU/UDH 是 fallback。发送短信不作为 1.0 承诺。eSIM 首选 Windows 11 LPA；仅在检测到真实 eUICC 通道时提供实验兼容安装。

1.0 新增发布级数据安全与诊断：本地 JSON 数据统一带 `.bak` 恢复，设置页可以运行兼容性自检并导出不含短信正文/激活码的诊断 JSON。发布 CI 会输出 MSI、NSIS EXE 和 SHA256 校验和。
