# CellularHub local-debug handoff

This file is the entry point for continuing CellularHub on a real Windows machine.

## Checkout

```powershell
git clone https://github.com/Junyxor/CellularHub.git
cd CellularHub
git checkout recovery/1.0-bootstrap
```

The recovery branch intentionally uses normal Git source files. Do not reintroduce the old zip/base64 bootstrap transfer.

## Current architecture

```text
React / Vite UI
      |
      | Tauri commands / events
      v
AppState --------------------------------------------------+
  |                                                       |
  +--> providers::windows  -> Windows MBN / IMbn*         |
  +--> providers::at       -> AT receive fallback         |
  +--> sms::pdu            -> GSM7 / UCS2 / PDU / UDH     |
  +--> sms::inbox          -> multipart / dedupe / classify
  +--> Store               -> messages.json / esim-profiles.json
```

The important design rule is capability honesty: seeing an MBN modem does not prove eSIM support, finding `lpac.exe` does not prove a real eUICC, and SMS send remains disabled until receive paths have been tested on multiple devices.

## What is already in place

- Tauri 2 + React/Vite application shell.
- Full and mini UI layouts with Liquid Glass settings.
- Local SMS/eSIM JSON persistence.
- Windows Mobile Broadband interface enumeration.
- Conservative SMS receive capability flag based on `MBN_SMS_CAPS_PDU_RECEIVE`.
- Windows `lpa:` activation handoff, gated by an installed handler.
- AT receive protocol primitives for `CMGF`, `CNMI`, `CMTI`, `CSQ`, and `COPS`.
- GSM 7-bit, UCS-2, 8-bit PDU decoding and 8/16-bit multipart UDH metadata.
- Unified inbox ingestion core for multipart assembly, duplicate suppression, categorization, verification-code extraction, and persistence.
- A debug-only Tauri command for manually feeding a raw SMS-DELIVER PDU through the real decode/persist path.

## The next implementation order

1. **Make CI green first.** Run `scripts/dev-check.ps1` and fix compilation/API binding errors before real-device debugging.
2. **Wire MBN receive.** Obtain `IMbnSms` from a real interface, call `SmsRead`, normalize every returned PDU, and pass it to `AppState::ingest_pdu`.
3. **Add MBN events.** Subscribe to `IMbnSmsEvents` through COM connection points. The callback should only enqueue work; decoding/persistence should happen outside the COM callback.
4. **Wire AT transport.** Enumerate COM ports, let the user choose one, own it exclusively, attempt text mode first and fall back to PDU mode.
5. **Handle `+CMTI`.** Run `AT+CMGR=<index>` and feed PDU responses to the same `AppState::ingest_pdu` path. Add `CMGL` only for startup catch-up.
6. **Emit UI updates.** After a stored message, emit `cellularhub:snapshot` (or a narrower SMS event) so the current React session updates without polling.
7. **Only after SMS is stable:** probe EID/eUICC read-only, then add guarded lpac APDU transport.

## Debug-only raw PDU path

`debug_ingest_sms_pdu` is compiled into the command table but refuses to run in release builds. It is intended to test this chain without a modem callback:

```text
raw PDU -> decode_deliver_pdu -> SmsInbox -> messages.json -> AppSnapshot
```

The TypeScript wrapper is `debugIngestSmsPdu()` in `src/lib/backend.ts`. Temporarily wire it to a dev-only button when testing captured PDUs.

## Important code entry points

- `src-tauri/src/providers/windows.rs` - MBN enumeration and Windows-native capability detection.
- `src-tauri/src/providers/at.rs` - AT parser/init primitives; serial ownership is not implemented yet.
- `src-tauri/src/sms/pdu.rs` - raw SMS-DELIVER decoder.
- `src-tauri/src/sms/inbox.rs` - multipart assembly, dedupe, classification, verification codes.
- `src-tauri/src/state.rs` - the correct persistence boundary for incoming messages.
- `src-tauri/src/store.rs` - local JSON storage and message retention cap.
- `src-tauri/src/lib.rs` - Tauri command registration and event emission.
- `docs/LOCAL_DEBUG.md` - step-by-step Windows workflow.
- `docs/HARDWARE_TEST_MATRIX.md` - what to test before claiming 1.0 hardware support.

## Do not do these yet

- Do not enable SMS send or add `AT+CMGS`.
- Do not label a device as eSIM-capable just because it is WWAN/MBIM.
- Do not run lpac install commands until a real EID and APDU path are proven.
- Do not perform blocking serial I/O or PDU parsing inside COM event callbacks.
- Do not delete SMS from the modem automatically during early testing; first prove dedupe and persistence.

## Definition of a useful local milestone

A good first local milestone is not “all eSIM features work.” It is:

> A real SIM receives one SMS; CellularHub obtains it through either MBN or AT, decodes Chinese/ASCII correctly, assembles multipart messages, writes one deduplicated entry to `messages.json`, and shows it in the running UI without restart.

Once that works reliably, the remaining provider and eSIM work becomes much easier to isolate.
