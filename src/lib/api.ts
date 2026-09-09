import { invoke } from '@tauri-apps/api/core';
import type { AtConnectRequest, CellularSnapshot, ConversationMeta, DeviceInfo, EsimCompatInstallRequest, EsimCompatReport, EsimProfile, ReleaseDiagnostics, SmsMessage, WindowMode } from './types';

const isTauri = () => '__TAURI_INTERNALS__' in window;

const demoSnapshot: CellularSnapshot = {
  backend: 'mock',
  deviceName: '开发预览模式',
  operatorName: 'CellularHub Demo',
  networkType: '5G',
  signalPercent: 82,
  rssiDbm: -78,
  connected: true,
  smsReady: true,
  esimReady: false,
};

const demoMessages: SmsMessage[] = [
  {
    id: 'demo-1',
    sender: '1069 5500',
    body: '【CellularHub】验证码 482631，5 分钟内有效。',
    receivedAt: new Date(Date.now() - 1000 * 60 * 3).toISOString(),
    unread: true,
    archived: false,
    backend: 'mock',
  },
  {
    id: 'demo-2',
    sender: '10010',
    body: '本月已使用流量 36.2 GB，当前套餐状态正常。',
    receivedAt: new Date(Date.now() - 1000 * 60 * 42).toISOString(),
    unread: false,
    archived: false,
    backend: 'mock',
  },
  {
    id: 'demo-3',
    sender: '1069 5500',
    body: '【CellularHub】登录验证码 731904，请勿向任何人泄露。',
    receivedAt: new Date(Date.now() - 1000 * 60 * 60 * 5).toISOString(),
    unread: false,
    archived: false,
    backend: 'mock',
  },
  {
    id: 'demo-4',
    sender: '10010',
    body: '您的套餐将于 9 月 18 日续费，本期费用 29.00 元。',
    receivedAt: new Date(Date.now() - 1000 * 60 * 60 * 22).toISOString(),
    unread: false,
    archived: false,
    backend: 'mock',
  },
 ];

let demoConversationMeta: ConversationMeta[] = [
  { sender: '1069 5500', alias: 'CellularHub', notes: 'Demo OTP service', pinned: true, updatedAt: new Date().toISOString() },
  { sender: '10010', alias: '\u4e2d\u56fd\u8054\u901a', notes: '\u5957\u9910\u4e0e\u6d41\u91cf\u901a\u77e5', pinned: false, updatedAt: new Date().toISOString() },
];

let demoProfiles: EsimProfile[] = [
  {
    id: 'demo-esim-jp', displayName: '日本保号卡', countryCode: 'JP', countryName: '日本', operatorName: 'Demo Mobile',
    phoneNumber: '+81 90 **** 1288', iccid: '', planName: '保号套餐', retentionCost: 3, currency: 'USD', billingCycle: '每月',
    expiryDate: '', renewalDate: new Date(Date.now() + 1000 * 60 * 60 * 24 * 6).toISOString().slice(0, 10), autoRenew: false,
    status: 'active', notes: '用于接收验证码。', tags: ['保号', '验证码'], customFields: [], createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(),
  },
];

export async function getSnapshot(): Promise<CellularSnapshot> {
  if (!isTauri()) return demoSnapshot;
  return invoke('get_snapshot');
}

export async function listDevices(): Promise<DeviceInfo[]> {
  if (!isTauri()) return [
    {
      id: 'mbn:{DEMO}', name: 'Windows Mobile Broadband (示例)', backend: 'windows_mbn', manufacturer: 'Fibocom', product: 'FM350',
      capabilities: { data: true, smsReceive: true, smsSend: false, esim: false, signal: true, operator: true },
    },
    {
      id: 'demo-at', name: 'Quectel RM520N-GL AT (示例)', backend: 'at', port: 'COM7', manufacturer: 'Quectel', product: 'RM520N-GL',
      capabilities: { data: false, smsReceive: true, smsSend: false, esim: false, signal: true, operator: true },
    },
  ];
  return invoke('list_devices');
}

export async function listMessages(): Promise<SmsMessage[]> {
  if (!isTauri()) return demoMessages;
  return invoke('list_messages');
}

export async function listConversationMeta(): Promise<ConversationMeta[]> {
  if (!isTauri()) return demoConversationMeta;
  return invoke('list_conversation_meta');
}

export async function saveConversationMeta(meta: ConversationMeta): Promise<ConversationMeta> {
  if (!isTauri()) {
    const saved = { ...meta, updatedAt: new Date().toISOString() };
    demoConversationMeta = [...demoConversationMeta.filter((item) => item.sender !== saved.sender), saved];
    return saved;
  }
  return invoke('save_conversation_meta', { meta });
}

export async function copyText(text: string): Promise<void> {
  if (!isTauri()) { await navigator.clipboard.writeText(text); return; }
  return invoke('copy_text', { text });
}

export async function connectAt(request: AtConnectRequest): Promise<void> {
  if (!isTauri()) return;
  return invoke('connect_at', { request });
}

export async function connectWindowsMbn(deviceId: string): Promise<void> {
  if (!isTauri()) return;
  return invoke('connect_windows_mbn', { deviceId });
}

export async function probeEsimCompat(): Promise<EsimCompatReport> {
  if (!isTauri()) return {
    windowsLpaAvailable: true,
    lpacPath: 'C:\\CellularHub\\tools\\lpac.exe',
    candidates: [{
      port: 'COM7', atApdu: true, atCsim: true, vendorEsim: true, eid: '89049032000000000000000000001234',
      lpacAvailable: true, installReady: true, note: 'CCHO/CGLA APDU · CSIM APDU · EID 可读 · lpac 已发现',
    }],
  };
  return invoke('probe_esim_compat');
}

export async function installEsimCompat(request: EsimCompatInstallRequest): Promise<string> {
  if (!isTauri()) return `预览模式：未向 ${request.port} 执行真实 eSIM 写入。`;
  return invoke('install_esim_compat', { request });
}

export async function disconnect(): Promise<void> {
  if (!isTauri()) return;
  return invoke('disconnect_backend');
}

export async function markRead(id: string): Promise<void> {
  if (!isTauri()) return;
  return invoke('mark_message_read', { id });
}

export async function markAllRead(): Promise<void> {
  if (!isTauri()) { demoMessages.forEach((message) => { if (!message.archived) message.unread = false; }); return; }
  return invoke('mark_all_messages_read');
}

export async function markConversationRead(sender: string): Promise<void> {
  if (!isTauri()) {
    demoMessages.forEach((message) => { if (message.sender === sender && !message.archived) message.unread = false; });
    return;
  }
  return invoke('mark_conversation_read', { sender });
}

export async function setConversationArchived(sender: string, archived: boolean): Promise<void> {
  if (!isTauri()) {
    demoMessages.forEach((message) => {
      if (message.sender === sender) {
        message.archived = archived;
        if (archived) message.unread = false;
      }
    });
    return;
  }
  return invoke('set_conversation_archived', { sender, archived });
}

export async function setMessageArchived(id: string, archived: boolean): Promise<void> {
  if (!isTauri()) {
    const message = demoMessages.find((item) => item.id === id);
    if (message) { message.archived = archived; if (archived) message.unread = false; }
    return;
  }
  return invoke('set_message_archived', { id, archived });
}

export async function updateTrayUnread(unread: number): Promise<void> {
  if (!isTauri()) return;
  return invoke('update_tray_unread', { unread });
}

export async function launchEsim(uri: string): Promise<void> {
  if (!isTauri()) return;
  return invoke('launch_esim_uri', { uri });
}

export async function injectDemoSms(): Promise<void> {
  if (!isTauri()) return;
  return invoke('inject_demo_sms');
}

export async function listEsimProfiles(): Promise<EsimProfile[]> {
  if (!isTauri()) return demoProfiles;
  return invoke('list_esim_profiles');
}

export async function saveEsimProfile(profile: EsimProfile): Promise<EsimProfile> {
  if (!isTauri()) {
    const now = new Date().toISOString();
    const saved = { ...profile, id: profile.id || crypto.randomUUID(), createdAt: profile.createdAt || now, updatedAt: now };
    demoProfiles = [...demoProfiles.filter((item) => item.id !== saved.id), saved];
    return saved;
  }
  return invoke('save_esim_profile', { profile });
}

export async function deleteEsimProfile(id: string): Promise<void> {
  if (!isTauri()) { demoProfiles = demoProfiles.filter((item) => item.id !== id); return; }
  return invoke('delete_esim_profile', { id });
}

export async function checkEsimReminders(): Promise<number> {
  if (!isTauri()) return 0;
  return invoke('check_esim_reminders');
}

export async function setWindowMode(mode: WindowMode): Promise<void> {
  if (!isTauri()) return;
  return invoke('set_window_mode', { mode });
}

export async function hideMainWindow(): Promise<void> {
  if (!isTauri()) return;
  return invoke('hide_main_window');
}

export async function showQuickPanelIfHidden(): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke('show_quick_panel_if_hidden');
}

export async function setCloseToTray(enabled: boolean): Promise<void> {
  if (!isTauri()) return;
  return invoke('set_close_to_tray', { enabled });
}

export async function getAutostartEnabled(): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke('get_autostart_enabled');
}

export async function setAutostartEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauri()) return enabled;
  return invoke('set_autostart_enabled', { enabled });
}

export async function runReleaseDiagnostics(): Promise<ReleaseDiagnostics> {
  if (!isTauri()) return {
    appVersion: '1.0.0', os: 'browser-preview', arch: 'web', dataDir: 'Preview mode',
    messageCount: demoMessages.length, unreadCount: demoMessages.filter((m) => m.unread && !m.archived).length,
    esimProfileCount: demoProfiles.length, conversationMetaCount: demoConversationMeta.length, snapshot: demoSnapshot,
    deviceCount: 2, generatedAt: new Date().toISOString(),
    checks: [
      { id: 'platform', label: 'Windows 平台', status: 'info', detail: '浏览器预览模式' },
      { id: 'storage', label: '本地数据目录', status: 'pass', detail: '发布版使用分阶段写入 + .bak 恢复' },
      { id: 'sms', label: 'SMS 接收后端', status: 'pass', detail: 'Demo: Windows MBN + AT fallback' },
      { id: 'send', label: 'SMS 发送', status: 'info', detail: '1.0 默认关闭；兼容层只承诺接收' },
    ],
  };
  return invoke('run_release_diagnostics');
}

export async function exportReleaseDiagnostics(): Promise<string> {
  if (!isTauri()) return 'Preview mode: diagnostics are not written to disk.';
  return invoke('export_release_diagnostics');
}
