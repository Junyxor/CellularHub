export type DeviceKind = 'windows-mbn' | 'at' | 'demo';
export type SmsCategory = 'code' | 'billing' | 'usage' | 'notice';
export type EsimStatus = 'active' | 'standby' | 'expired';

export interface DeviceCapabilities {
  smsReceive: boolean;
  smsSend: boolean;
  nativeEsim: boolean;
  euiccBridge: boolean;
}

export interface CellularDevice {
  id: string;
  label: string;
  kind: DeviceKind;
  model?: string;
  operator?: string;
  signal?: number;
  networkClass?: string;
  status: 'connected' | 'available' | 'unavailable';
  capabilities: DeviceCapabilities;
}

export interface SmsMessage {
  id: string;
  sender: string;
  body: string;
  receivedAt: string;
  unread: boolean;
  archived: boolean;
  category: SmsCategory;
  code?: string;
}

export interface EsimProfile {
  id: string;
  name: string;
  countryCode: string;
  operator: string;
  phoneNumber?: string;
  iccid?: string;
  plan?: string;
  keepAliveCost?: number;
  currency?: string;
  billingCycle?: string;
  renewalDate?: string;
  expiryDate?: string;
  autoRenew: boolean;
  status: EsimStatus;
  notes?: string;
  tags: string[];
  updatedAt: string;
}

export interface RuntimeCapabilities {
  nativeLpaAvailable: boolean;
  euiccBridgeAvailable: boolean;
  lpacFound: boolean;
  platform: string;
}

export interface AppSnapshot {
  devices: CellularDevice[];
  selectedDeviceId?: string;
  messages: SmsMessage[];
  esimProfiles: EsimProfile[];
  runtime: RuntimeCapabilities;
}
