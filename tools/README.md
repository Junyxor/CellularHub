# Optional lpac bridge

This directory intentionally does not contain an lpac binary.

For CellularHub's **experimental external-eUICC compatibility path**, place an official `lpac.exe` here (or beside `CellularHub.exe`, or on `PATH`). CellularHub will detect it automatically.

The AT APDU backends used by lpac (`at` / `at_csim`) are experimental and require a modem/eUICC that really exposes CCHO/CGLA or CSIM. A modem merely accepting a vendor eSIM command is not sufficient proof of an installed eUICC.

CellularHub does not use this bridge when Windows native LPA is available unless the user explicitly chooses the experimental external-eUICC action.
