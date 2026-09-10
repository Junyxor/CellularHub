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

## Native slice implemented after recovery

- Corrected the Windows Rust binding feature from `WindowsConnectionManager` to `MobileBroadband`.
- `IMbnInterfaceManager::GetInterfaces` now enumerates real Windows Mobile Broadband interfaces.
- MBN interface capability data now exposes manufacturer/model, registration/provider, data class and signal strength.
- `smsReceive` is only true when the interface advertises `MBN_SMS_CAPS_PDU_RECEIVE`; outbound SMS remains disabled.
- COM apartment ownership, SAFEARRAY destruction and capability BSTR cleanup are explicit so repeated refreshes do not leak native allocations.
- `lpa:` launch no longer goes through `cmd /C start`; Windows Explorer receives the URI directly.
- AT fallback protocol primitives now parse `+CMTI`, `+CSQ`, `+COPS` and define text/PDU receive-only initialization sequences.
- Shared SMS PDU code now covers GSM 7-bit unpacking, UCS-2, numeric/alphanumeric senders, 8-bit payloads, timestamps and 8/16-bit concatenation UDH metadata.
- Parser/unit tests are included so CI can catch regressions before modem-specific testing.

## Hardware work still deliberately incomplete

1. Wire `IMbnSms::SmsRead` and `IMbnSmsEvents` connection-point callbacks into the persistent SMS store.
2. Add Windows COM-port discovery/ownership and execute the AT CMGF/CNMI/CMTI/CMGR/CMGL receive state machine.
3. Add multipart assembly across received PDUs and duplicate suppression before persistence/notification.
4. Probe real eUICC/EID capability read-only and only then enable guarded lpac APDU transport.
5. Finish tray/autostart/single-instance and native notification routing.
6. Validate MBN and AT paths on multiple real modems before exposing SMS send.

The UI capability badges remain conservative: MBN presence does not imply eSIM support, and lpac being present does not imply a real eUICC bridge exists.
