export type BackendKind = 'mock' | 'at' | 'windows_mbn' | 'android' | 'router';
export type WindowMode = 'full' | 'mini';

export interface CapabilitySet {
  data: boolean;
  smsReceive: boolean;
  smsSend: boolean;
  esim: boolean;
  signal: boolean;
  operator: boolean;
}

export interface DeviceInfo {
  id: string;
  name: string;
  backend: BackendKind;
  port?: string;
  manufacturer?: string;
  product?: string;
  capabilities: CapabilitySet;
}

export interface CellularSnapshot {
  backend: BackendKind;
  deviceName: string;
  operatorName: string;
  networkType: string;
  signalPercent: number;
  rssiDbm?: number;
  connected: boolean;
  smsReady: boolean;
  esimReady: boolean;
  lastError?: string;
}

export interface SmsMessage {
  id: string;
  sender: string;
  body: string;
  receivedAt: string;
  unread: boolean;
  archived: boolean;
  backend: BackendKind;
  slot?: number;
}

export interface ConversationMeta {
  sender: string;
  alias: string;
  notes: string;
  pinned: boolean;
  updatedAt: string;
}

export interface AtConnectRequest {
  port: string;
  baudRate: number;
}

export interface EsimCustomField {
  id: string;
  label: string;
  value: string;
}

export interface EsimProfile {
  id: string;
  displayName: string;
  countryCode: string;
  countryName: string;
  operatorName: string;
  phoneNumber: string;
  iccid: string;
  planName: string;
  retentionCost: number;
  currency: string;
  billingCycle: string;
  expiryDate?: string;
  renewalDate?: string;
  autoRenew: boolean;
  status: string;
  notes: string;
  tags: string[];
  customFields: EsimCustomField[];
  createdAt: string;
  updatedAt: string;
}

export interface EsimCompatCandidate {
  port: string;
  atApdu: boolean;
  atCsim: boolean;
  vendorEsim: boolean;
  eid?: string;
  lpacAvailable: boolean;
  installReady: boolean;
  note: string;
}

export interface EsimCompatReport {
  windowsLpaAvailable: boolean;
  lpacPath?: string;
  candidates: EsimCompatCandidate[];
}

export interface EsimCompatInstallRequest {
  port: string;
  activationCode: string;
  transport: 'at' | 'at_csim';
}


export interface HealthCheckItem {
  id: string;
  label: string;
  status: 'pass' | 'warn' | 'fail' | 'info' | string;
  detail: string;
}

export interface ReleaseDiagnostics {
  appVersion: string;
  os: string;
  arch: string;
  dataDir: string;
  messageCount: number;
  unreadCount: number;
  esimProfileCount: number;
  conversationMetaCount: number;
  snapshot: CellularSnapshot;
  deviceCount: number;
  checks: HealthCheckItem[];
  generatedAt: string;
}
