# CellularHub hardware validation matrix

Do not claim a hardware path as 1.0-ready until the corresponding row has been tested on real devices.

| Area | Case | Expected result | Status |
| --- | --- | --- | --- |
| Build | `npm run build` | TypeScript/Vite succeeds | pending local/CI |
| Build | `cargo test` | Rust parser/state tests succeed | pending local/CI |
| Build | `cargo check` on Windows | Windows bindings compile | pending local/CI |
| UI | no modem attached | app starts, no fake capabilities | ready for test |
| UI | full -> mini -> full | state remains intact | ready for test |
| MBN | enumerate built-in WWAN | real device/model/provider shown | implementation present |
| MBN | modem lacks PDU receive | `smsReceive=false` | implementation present |
| MBN | modem advertises PDU receive | `smsReceive=true` | implementation present |
| MBN | `IMbnSms::SmsRead` | unread SMS enters unified inbox | **not implemented** |
| MBN | new-SMS COM event | background receive without polling | **not implemented** |
| AT | passive COM discovery | candidate shown but not auto-opened | implementation present |
| AT | wrong COM port | failure is explicit; no fake connected state | ready for test |
| AT | text-mode modem | `CMGF=1` receive succeeds | implementation present, needs hardware |
| AT | PDU-only modem | fallback to `CMGF=0` succeeds | implementation present, needs hardware |
| AT | `+CMTI` | indexed SMS is read with `CMGR` | implementation present, needs hardware |
| AT | no `+CMTI` | periodic unread `CMGL` catches message | implementation present, needs hardware |
| AT | background listener restart | stable USB identity restores correct COM port | implementation present, needs hardware |
| SMS | GSM 7-bit | ASCII content correct | unit coverage + hardware needed |
| SMS | UCS-2 | Chinese content correct | unit coverage + hardware needed |
| SMS | long 8-bit-ref multipart | one ordered message persisted | unit coverage + hardware needed |
| SMS | long 16-bit-ref multipart | one ordered message persisted | unit coverage + hardware needed |
| SMS | repeated modem poll | no duplicate entry | implementation present |
| SMS | OTP message | code extracted and category=`code` | implementation present |
| SMS | unrelated 4-8 digits | no OTP extraction without semantic hint | implementation present |
| Store | >1000 messages | newest 1000 retained | implementation present |
| eSIM | Windows without `lpa:` handler | native install unavailable | implementation present |
| eSIM | Windows with `lpa:` handler | activation code handed to Windows | implementation present, needs hardware/OS |
| eSIM | `lpac.exe` only, no eUICC proof | bridge remains unavailable | implementation present |
| eSIM | real EID/APDU transport | bridge may become available | **not implemented** |
| SMS send | any modem | send stays unavailable | intentional |

## Minimum device coverage before 1.0

Try to cover at least:

1. one built-in Windows WWAN/MBIM laptop modem;
2. one USB LTE/5G modem exposing AT ports;
3. one modem that accepts text-mode SMS;
4. one modem or firmware path that requires PDU mode;
5. Chinese UCS-2, ASCII GSM7 and a multipart SMS;
6. suspend/resume and USB unplug/replug while the app is running.

## Failure policy

A hardware test failure should degrade capability/status and surface an actionable error. It must not silently flip a capability flag to true, manufacture a synthetic eUICC, or delete unread modem messages during bring-up.
