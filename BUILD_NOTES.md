# CellularHub 1.0 recovery notes

The repository recovery intentionally avoids the failed base64 bootstrap transfer.
All application source is stored as ordinary Git files.

## Recovery scope now present

- React/Vite full control center and independent mini layout.
- Liquid Glass appearance controls shared through localStorage.
- SMS conversation UI, archive/read commands and local persisted messages.json.
- eSIM Wallet-style profile management persisted to esim-profiles.json.
- Native-first capability model; no simulated eUICC on unsupported hardware.
- Windows LPA activation-code handoff.
- Tauri/Rust command bridge and app-data store.
- Windows CI for frontend build, cargo check/test and MSI/NSIS bundles.

## Deliberately incomplete hardware work

The recovery bootstrap does not pretend that Windows MBN, AT SMS or lpac APDU transport is already wired when it is not. The next implementation slice is:

1. IMbnInterfaceManager / IMbnSms enumeration and PDU receive events.
2. AT COM enumeration + CMGF/CNMI/CMTI/CMGR/CMGL fallback.
3. Shared GSM 7-bit/UCS-2/PDU/UDH decoder and multipart assembler.
4. Read-only eUICC capability probing and guarded lpac integration.
5. Tray/autostart/single-instance and notification routing.

This keeps UI capability badges truthful while the native providers are implemented and tested on real hardware.
