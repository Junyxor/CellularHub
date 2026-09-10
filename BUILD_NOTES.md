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
- Windows serial/USB candidates are now enumerated passively as AT fallback devices. Port presence alone does not mark SMS as supported.
- An explicit `poll_sms_device` command opens an AT candidate, probes common modem baud rates, reads operator/signal, enables text SMS when possible and falls back to PDU mode otherwise.
- The AT path executes receive-only `CMGF`, `CNMI` and `CMGL`; there is no `CMGS` send command.
- Text-mode `+CMGL` records and PDU-mode records are converted into one `IncomingSms` representation.
- Shared PDU code covers GSM 7-bit, UCS-2, numeric/alphanumeric senders, 8-bit payloads and 8/16-bit concatenation UDH metadata.
- Multipart SMS is assembled by sender/reference/part count before persistence. Incomplete groups expire after 12 hours.
- Provider records receive deterministic UUID v5 identities, so repeated polling does not duplicate already persisted SMS.
- Verification-code extraction is gated on local semantic keywords before accepting 4-8 digit runs; billing/renewal and usage categories are also classified locally.
- `messages.json` remains newest-first and now correctly retains the newest 1000 entries rather than the oldest tail.

## Hardware work still deliberately incomplete

1. Wire `IMbnSms::SmsRead` and the `IMbnSmsEvents` connection-point completion sink into the same `IncomingSms` pipeline. MBN reads are asynchronous and are not faked as synchronous calls.
2. Promote the explicit AT poll into a long-lived receive session that reacts to `+CMTI` and uses `CMGR` immediately instead of relying only on `CMGL` polling.
3. Add UI controls for explicitly selecting/testing an AT port and surface provider diagnostics without automatically stealing arbitrary COM ports.
4. Probe real eUICC/EID capability read-only and only then enable guarded lpac APDU transport.
5. Finish tray/autostart/single-instance and native notification routing.
6. Validate MBN and AT paths on multiple real modems before exposing SMS send.

The UI capability badges remain conservative: MBN presence does not imply eSIM support, serial-port presence does not imply SMS support, and lpac being present does not imply a real eUICC bridge exists.
