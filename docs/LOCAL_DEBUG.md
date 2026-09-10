# Windows local debugging guide

Use this guide on a real Windows 11 machine. The goal is to prove one receive path at a time rather than debugging UI, MBN, serial, PDU and eSIM simultaneously.

## 1. Prerequisites

Install:

- Git
- Node.js 22+
- Rust stable via rustup
- Visual Studio 2022 Build Tools with **Desktop development with C++**
- WebView2 Runtime

Then clone and switch to the handoff branch:

```powershell
git clone https://github.com/Junyxor/CellularHub.git
cd CellularHub
git checkout recovery/1.0-bootstrap
```

## 2. Run the non-hardware checks first

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\dev-check.ps1
```

If that fails, fix the compiler/build error before opening a modem port.

Useful individual commands:

```powershell
npm install
npm run build
cargo test --manifest-path .\src-tauri\Cargo.toml
cargo check --manifest-path .\src-tauri\Cargo.toml
npm run tauri dev
```

## 3. Inspect what Windows can see

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\modem-diagnostics.ps1
```

The script is passive: it does not open COM ports or send AT commands. Keep the output when reporting a hardware bug.

## 4. Start with the AT path when a modem exposes a serial AT port

The AT path is currently the most complete real receive path.

1. Run `npm run tauri dev`.
2. Open **设备**.
3. Find the AT/COM candidate that corresponds to the modem.
4. Click **测试并读取短信** first.
5. If the test succeeds, send the SIM a real SMS.
6. Click the test/read action again and verify that exactly one message is persisted.
7. Then try **开启后台接收** and send another SMS.
8. Verify that the UI updates without restarting the app.

Do not open the same COM port in PuTTY, serial terminals, vendor tools or another CellularHub instance while the background listener owns it.

## 5. What to inspect after a received SMS

CellularHub stores local data through Tauri's app-data directory. With the current identifier, the expected Windows location is under:

```text
%APPDATA%\dev.junyxor.cellularhub\
```

The important files are:

```text
messages.json
esim-profiles.json
preferences.json
```

For one test SMS, verify:

- sender is correct;
- Chinese/ASCII body is correct;
- `receivedAt` is plausible;
- `unread` is true on first arrival;
- a repeated poll does not create a duplicate;
- multipart SMS becomes one final entry;
- a verification code is only extracted when semantic hints such as `验证码` / `OTP` are present.

## 6. MBN path status

Windows MBN enumeration and SMS capability detection are implemented, but **MBN message read/events are not yet wired**. Therefore:

- seeing a `windows-mbn` device is expected;
- seeing `SMS 接收` capability is possible when Windows reports `MBN_SMS_CAPS_PDU_RECEIVE`;
- actual MBN SMS delivery is the main unfinished native task;
- do not treat an MBN device as eSIM-capable unless a separate eUICC/LPA capability proves it.

The next MBN work should be isolated in `src-tauri/src/providers/windows.rs` and should feed received PDU records into the same `IncomingSms`/`AppState` path already used by AT.

## 7. Debug strategy for MBN

When implementing `IMbnSms`:

1. Keep COM calls on a thread with an explicit apartment.
2. Treat `SmsRead` as asynchronous; do not wrap it as if it synchronously returned the final PDU list.
3. Implement the proper completion/event sink.
4. Copy returned data into Rust-owned values inside the COM boundary.
5. Queue those values to normal Rust code.
6. Decode/persist outside the COM callback.
7. Emit the existing `cellularhub:sms-received` event only after persistence succeeds.

This avoids holding COM-owned pointers or the app state mutex across callbacks.

## 8. AT debugging notes

If AT probing fails, check in this order:

- wrong COM port;
- port already owned by another program;
- unsupported baud rate;
- modem requires a different SMS storage (`SM`, `ME`, etc.);
- modem supports PDU but not text mode;
- `CNMI` mode rejected by firmware;
- SMS arrives but `CMTI` is not emitted, in which case the periodic `CMGL` fallback should still discover unread messages.

Do not add `AT+CMGS` while debugging receive. Outbound SMS is intentionally disabled for 1.0 bring-up.

## 9. Minimal bug report template

When something fails locally, capture:

```text
Windows version:
Laptop/modem model:
Connection type: built-in WWAN / USB modem / other
CellularHub branch + commit:
Device shown in UI:
COM port (if AT):
Action: poll / background listener / MBN
Expected:
Actual:
Relevant terminal/Rust error:
Does the modem receive the same SMS on another known-working tool?: yes/no
```

Also attach the passive `scripts/modem-diagnostics.ps1` output. Do not attach private SMS contents unless they are test messages you intentionally created.
