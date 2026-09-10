# CellularHub 1.0 recovery notes

The repository recovery intentionally avoids the failed base64 bootstrap transfer.
All application source is stored as ordinary Git files.

## Recovery scope now present

- React/Vite full control center and independent mini layout.
- Liquid Glass appearance controls shared through localStorage.
- SMS conversation UI, archive/read commands and local persisted messages.json.
- eSIM Wallet-style profile management persisted to esim-profiles.json.
- Native-first capability model; no simulated eUICC on unsupported hardware.
- Windows 11 24H2+ `lpa:` activation-code handoff, gated by an installed URI handler.
- Tauri/Rust command bridge and app-data store.
- Windows CI for frontend build, cargo check/test and MSI/NSIS bundles.

## Native / receive pipeline implemented after recovery

- Corrected the Windows Rust binding feature from `WindowsConnectionManager` to `MobileBroadband`.
- `IMbnInterfaceManager::GetInterfaces` enumerates real Windows Mobile Broadband interfaces.
- MBN capability data exposes manufacturer/model, registration/provider, data class and signal strength.
- `smsReceive` is only true when the MBN interface advertises `MBN_SMS_CAPS_PDU_RECEIVE`; outbound SMS remains disabled.
- COM apartment ownership, SAFEARRAY destruction and capability BSTR cleanup are explicit.
- `lpa:` launch no longer goes through `cmd /C start`; Windows Explorer receives the URI directly.
- Windows serial/USB candidates are enumerated passively as AT fallback devices. Port presence alone does not mark SMS as supported.
- Explicit AT testing opens the chosen port, probes common baud rates, reads operator/signal, enables text SMS when possible and falls back to PDU mode otherwise.
- The AT path executes receive-only `CMGF`, `CNMI`, `CMGL` and `CMGR`; there is no `CMGS` send command.
- A long-lived AT worker can own the explicitly selected COM port, react to `+CMTI`, immediately read the indexed message with `CMGR`, and fall back to an unread `CMGL` scan every 30 seconds.
- Starting a different AT listener first stops and joins the previous worker, so CellularHub has only one serial SMS owner at a time.
- Background provider messages feed the same multipart assembler, deterministic duplicate filter, classifier and `messages.json` persistence path as manual polling.
- Tauri emits `cellularhub:sms-received` after a newly persisted background SMS; full and mini UI subscribe to the same snapshot refresh path.
- The device page exposes explicit `测试并读取短信`, `开启后台接收` and `停止后台接收`; arbitrary serial ports are never opened merely because they were discovered.
- Text-mode records and PDU-mode records are converted into one `IncomingSms` representation.
- Shared PDU code covers GSM 7-bit, UCS-2, numeric/alphanumeric senders, 8-bit payloads and 8/16-bit concatenation UDH metadata.
- Multipart SMS is assembled by sender/reference/part count before persistence. Incomplete groups expire after 12 hours.
- Provider records receive deterministic UUID v5 identities, so repeated polling does not duplicate already persisted SMS.
- Verification-code extraction is gated on local semantic keywords before accepting 4-8 digit runs; billing/renewal and usage categories are also classified locally.
- `messages.json` remains newest-first and correctly retains the newest 1000 entries.
- Receive preferences are saved to local `preferences.json`. Successful explicit tests remember a device only when it has a stable identity.
- Background-receive consent is persisted only for USB AT devices that expose VID/PID plus a non-empty serial number. A bare `COM3`-style identity is never sufficient for auto-open.
- On restart, the saved physical USB identity is resolved back to the current COM number. The listener is restored only after that stable match, so COM renumbering is supported without risking ownership of an unrelated serial device.
- Explicitly stopping background receive clears the persisted auto-receive consent. Restore failures do not block application startup.
- Fixed a listener-state self-deadlock where asking to start an already active device could call `snapshot()` while holding the listener mutex.
- Windows bundling explicitly uses `src-tauri/icons/icon.ico`; the earlier empty `bundle.icon` recovery setting no longer hides the restored icon from WiX/NSIS.

## Hardware work still deliberately incomplete

1. Wire `IMbnSms::SmsRead` and the `IMbnSmsEvents` connection-point completion sink into the same `IncomingSms` pipeline. MBN reads are asynchronous and are not faked as synchronous calls.
2. Decode AT text-mode SCTS timezone exactly instead of replacing it with ingestion time.
3. Probe real eUICC/EID capability read-only and only then enable guarded lpac APDU transport.
4. Finish tray/autostart/single-instance and native notification routing.
5. Validate MBN and AT paths on multiple real modems before exposing SMS send.

The UI capability badges remain conservative: MBN presence does not imply eSIM support, serial-port presence does not imply SMS support, and lpac being present does not imply a real eUICC bridge exists.
