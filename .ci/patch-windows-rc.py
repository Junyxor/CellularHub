from pathlib import Path

# Temporary CI-only materialization for the RC branch. The final source commit
# will carry these fixes directly once the Windows compile gate is green.

icons = Path('src-tauri/icons')
icons.mkdir(parents=True, exist_ok=True)
badge = (icons / '32x32.png').read_bytes()
for n in range(1, 10):
    (icons / f'tray-unread-{n}.png').write_bytes(badge)
(icons / 'tray-unread-9plus.png').write_bytes(badge)

cargo = Path('src-tauri/Cargo.toml')
text = cargo.read_text(encoding='utf-8')
if 'windows-core = "0.62.2"' not in text:
    needle = '[target.\'cfg(windows)\'.dependencies]\nwindow-vibrancy = "0.8.0"\n'
    if needle not in text:
        raise SystemExit('unexpected Cargo.toml Windows dependency block')
    text = text.replace(needle, needle + 'windows-core = "0.62.2"\n', 1)
cargo.write_text(text, encoding='utf-8')

mbn = Path('src-tauri/src/backend/windows_mbn.rs')
text = mbn.read_text(encoding='utf-8')
old = '''    use windows::{
        core::{implement, Interface, IUnknown, PCWSTR, Ref, Result as WinResult, HRESULT},'''
new = '''    use windows_core::implement;
    use windows::{
        core::{Interface, IUnknown, PCWSTR, Ref, Result as WinResult, HRESULT},'''
if old in text:
    text = text.replace(old, new, 1)
text = text.replace('Foundation::{SAFEARRAY, VARIANT_BOOL},', 'Foundation::VARIANT_BOOL,', 1)
old_com = '''                    CoCreateInstance, CoInitializeEx, CoUninitialize, IConnectionPointContainer,
                    CLSCTX_ALL, COINIT_MULTITHREADED,'''
new_com = '''                    CoCreateInstance, CoInitializeEx, CoUninitialize, IConnectionPointContainer,
                    SAFEARRAY, CLSCTX_ALL, COINIT_MULTITHREADED,'''
if old_com in text:
    text = text.replace(old_com, new_com, 1)

required = [
    'use windows_core::implement;',
    'Foundation::VARIANT_BOOL,',
    'SAFEARRAY, CLSCTX_ALL, COINIT_MULTITHREADED,',
]
for marker in required:
    if marker not in text:
        raise SystemExit(f'MBN patch marker missing: {marker}')
mbn.write_text(text, encoding='utf-8')

print('RC patch: tray badges materialized; windows-core 0.62.2 + MBN imports aligned')
