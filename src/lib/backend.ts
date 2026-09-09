import { invoke } from '@tauri-apps/api/core';
import { createDemoSnapshot } from './demo';
import type { AppSnapshot, EsimProfile } from '../types';

let browserSnapshot = createDemoSnapshot();

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function getSnapshot(): Promise<AppSnapshot> {
  if (!isTauriRuntime()) return structuredClone(browserSnapshot);
  return invoke<AppSnapshot>('get_snapshot');
}

export async function setWindowMode(mode: 'full' | 'mini'): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke('set_window_mode', { mode });
}

export async function archiveMessage(id: string, archived: boolean): Promise<AppSnapshot> {
  if (!isTauriRuntime()) {
    browserSnapshot = {
      ...browserSnapshot,
      messages: browserSnapshot.messages.map((message) =>
        message.id === id ? { ...message, archived, unread: archived ? false : message.unread } : message,
      ),
    };
    return structuredClone(browserSnapshot);
  }
  return invoke<AppSnapshot>('archive_message', { id, archived });
}

export async function markSenderRead(sender: string): Promise<AppSnapshot> {
  if (!isTauriRuntime()) {
    browserSnapshot = {
      ...browserSnapshot,
      messages: browserSnapshot.messages.map((message) =>
        message.sender === sender ? { ...message, unread: false } : message,
      ),
    };
    return structuredClone(browserSnapshot);
  }
  return invoke<AppSnapshot>('mark_sender_read', { sender });
}

export async function upsertEsimProfile(profile: EsimProfile): Promise<AppSnapshot> {
  if (!isTauriRuntime()) {
    const exists = browserSnapshot.esimProfiles.some((item) => item.id === profile.id);
    browserSnapshot = {
      ...browserSnapshot,
      esimProfiles: exists
        ? browserSnapshot.esimProfiles.map((item) => (item.id === profile.id ? profile : item))
        : [profile, ...browserSnapshot.esimProfiles],
    };
    return structuredClone(browserSnapshot);
  }
  return invoke<AppSnapshot>('upsert_esim_profile', { profile });
}

export async function deleteEsimProfile(id: string): Promise<AppSnapshot> {
  if (!isTauriRuntime()) {
    browserSnapshot = {
      ...browserSnapshot,
      esimProfiles: browserSnapshot.esimProfiles.filter((profile) => profile.id !== id),
    };
    return structuredClone(browserSnapshot);
  }
  return invoke<AppSnapshot>('delete_esim_profile', { id });
}

export async function installEsimActivationCode(code: string): Promise<string> {
  if (!isTauriRuntime()) return `Demo: would hand ${code.slice(0, 18)}… to the platform LPA`;
  return invoke<string>('install_esim_activation_code', { code });
}

export async function refreshDevices(): Promise<AppSnapshot> {
  if (!isTauriRuntime()) return structuredClone(browserSnapshot);
  return invoke<AppSnapshot>('refresh_devices');
}
