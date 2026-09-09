import { useEffect, useMemo, useState } from 'react';
import {
  Archive,
  Bell,
  ChevronRight,
  CircleGauge,
  Copy,
  CreditCard,
  Database,
  Gauge,
  LayoutDashboard,
  MessageSquareText,
  MonitorSmartphone,
  PanelRightOpen,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  Signal,
  Smartphone,
  Trash2,
  WalletCards,
  Wifi,
} from 'lucide-react';
import {
  archiveMessage,
  deleteEsimProfile,
  getSnapshot,
  installEsimActivationCode,
  markSenderRead,
  refreshDevices,
  setWindowMode,
  upsertEsimProfile,
} from './lib/backend';
import type { AppSnapshot, EsimProfile, SmsMessage } from './types';

type Page = 'overview' | 'messages' | 'esim' | 'devices' | 'settings';
type Accent = 'azure' | 'violet' | 'mint' | 'rose';

interface Appearance {
  glass: number;
  tint: number;
  motion: boolean;
  semantic: boolean;
  accent: Accent;
}

const defaultAppearance: Appearance = {
  glass: 72,
  tint: 34,
  motion: true,
  semantic: true,
  accent: 'azure',
};

const nav: Array<{ page: Page; label: string; icon: typeof LayoutDashboard }> = [
  { page: 'overview', label: '概览', icon: LayoutDashboard },
  { page: 'messages', label: '短信', icon: MessageSquareText },
  { page: 'esim', label: 'eSIM', icon: WalletCards },
  { page: 'devices', label: '设备', icon: MonitorSmartphone },
  { page: 'settings', label: '设置与体验', icon: Settings2 },
];

function loadAppearance(): Appearance {
  try {
    const raw = localStorage.getItem('cellularhub.appearance');
    return raw ? { ...defaultAppearance, ...JSON.parse(raw) } : defaultAppearance;
  } catch {
    return defaultAppearance;
  }
}

function flagEmoji(countryCode: string): string {
  const code = countryCode.trim().toUpperCase();
  if (!/^[A-Z]{2}$/.test(code)) return '🌐';
  return String.fromCodePoint(...[...code].map((char) => 127397 + char.charCodeAt(0)));
}

function timeLabel(value: string): string {
  const date = new Date(value);
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  return sameDay
    ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    : date.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

function dateDistance(value?: string): string {
  if (!value) return '未设置';
  const days = Math.ceil((new Date(value).getTime() - Date.now()) / 86_400_000);
  if (days < 0) return `已过期 ${Math.abs(days)} 天`;
  if (days === 0) return '今天';
  return `${days} 天后`;
}

function categoryLabel(message: SmsMessage): string {
  return { code: '验证码', billing: '账单 / 续费', usage: '流量 / 套餐', notice: '通知' }[message.category];
}

function useAppearance() {
  const [appearance, setAppearance] = useState<Appearance>(loadAppearance);
  useEffect(() => {
    localStorage.setItem('cellularhub.appearance', JSON.stringify(appearance));
    const root = document.documentElement;
    root.dataset.accent = appearance.accent;
    root.dataset.motion = appearance.motion ? 'on' : 'off';
    root.dataset.semantic = appearance.semantic ? 'on' : 'off';
    root.style.setProperty('--glass-strength', `${appearance.glass}%`);
    root.style.setProperty('--glass-blur', `${10 + appearance.glass * 0.22}px`);
    root.style.setProperty('--tint-strength', `${appearance.tint / 100}`);
  }, [appearance]);
  return [appearance, setAppearance] as const;
}

export default function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [page, setPage] = useState<Page>('overview');
  const [loading, setLoading] = useState(true);
  const [mini, setMini] = useState(() => new URLSearchParams(location.search).get('mini') === '1');
  const [appearance, setAppearance] = useAppearance();

  useEffect(() => {
    getSnapshot()
      .then(setSnapshot)
      .finally(() => setLoading(false));
  }, []);

  const setMode = async (nextMini: boolean) => {
    setMini(nextMini);
    await setWindowMode(nextMini ? 'mini' : 'full');
  };

  if (loading || !snapshot) {
    return <div className="boot-screen"><div className="spinner" />正在连接蜂窝核心…</div>;
  }

  if (mini) {
    return <MiniPanel snapshot={snapshot} onExpand={() => setMode(false)} />;
  }

  const unread = snapshot.messages.filter((message) => message.unread && !message.archived).length;

  return (
    <div className="app-shell">
      <aside className="sidebar glass-heavy">
        <div className="brand">
          <div className="brand-mark"><Signal size={20} /></div>
          <div><strong>CellularHub</strong><span>Windows cellular center</span></div>
        </div>
        <nav>
          {nav.map(({ page: itemPage, label, icon: Icon }) => (
            <button key={itemPage} className={page === itemPage ? 'nav-item active' : 'nav-item'} onClick={() => setPage(itemPage)}>
              <Icon size={18} />
              <span>{label}</span>
              {itemPage === 'messages' && unread > 0 ? <b>{unread > 9 ? '9+' : unread}</b> : null}
            </button>
          ))}
        </nav>
        <div className="sidebar-status glass-soft">
          <span className="status-dot" />
          <div><strong>本地优先</strong><small>短信正文不会上传云端</small></div>
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div>
            <p className="eyebrow">CELLULAR CONTROL CENTER</p>
            <h1>{nav.find((item) => item.page === page)?.label}</h1>
          </div>
          <div className="top-actions">
            <button className="icon-button glass-soft" title="迷你模式" onClick={() => setMode(true)}><PanelRightOpen size={18} /></button>
            <button className="icon-button glass-soft" title="通知"><Bell size={18} />{unread > 0 ? <i /> : null}</button>
          </div>
        </header>

        <div className="page-body">
          {page === 'overview' && <Overview snapshot={snapshot} onNavigate={setPage} />}
          {page === 'messages' && <Messages snapshot={snapshot} onSnapshot={setSnapshot} />}
          {page === 'esim' && <EsimPage snapshot={snapshot} onSnapshot={setSnapshot} />}
          {page === 'devices' && <Devices snapshot={snapshot} onSnapshot={setSnapshot} />}
          {page === 'settings' && <Settings appearance={appearance} onChange={setAppearance} snapshot={snapshot} />}
        </div>
      </main>
    </div>
  );
}

function Overview({ snapshot, onNavigate }: { snapshot: AppSnapshot; onNavigate: (page: Page) => void }) {
  const device = snapshot.devices.find((item) => item.id === snapshot.selectedDeviceId) ?? snapshot.devices[0];
  const recent = snapshot.messages.filter((message) => !message.archived).slice(0, 3);
  const nextRenewal = [...snapshot.esimProfiles]
    .filter((profile) => profile.renewalDate || profile.expiryDate)
    .sort((a, b) => new Date(a.renewalDate ?? a.expiryDate!).getTime() - new Date(b.renewalDate ?? b.expiryDate!).getTime())[0];

  return (
    <div className="overview-grid">
      <section className="hero-card glass-heavy">
        <div className="hero-copy">
          <span className="pill success"><Wifi size={14} />{device?.networkClass ?? '蜂窝网络'}</span>
          <h2>{device?.operator ?? '等待蜂窝设备'}</h2>
          <p>{device?.model ?? 'Windows Native 与 AT 兼容层会按能力自动降级。'}</p>
          <div className="hero-actions">
            <button className="primary" onClick={() => onNavigate('devices')}>管理设备<ChevronRight size={16} /></button>
            <button className="secondary" onClick={() => onNavigate('messages')}>查看短信</button>
          </div>
        </div>
        <div className="signal-ring" style={{ '--signal': `${device?.signal ?? 0}%` } as React.CSSProperties}>
          <div><strong>{device?.signal ?? 0}</strong><span>% 信号</span></div>
        </div>
      </section>

      <section className="metric glass-card"><MessageSquareText /><div><span>未读短信</span><strong>{snapshot.messages.filter((m) => m.unread && !m.archived).length}</strong></div></section>
      <section className="metric glass-card"><CreditCard /><div><span>eSIM 档案</span><strong>{snapshot.esimProfiles.length}</strong></div></section>
      <section className="metric glass-card"><ShieldCheck /><div><span>SMS 接收</span><strong>{device?.capabilities.smsReceive ? '可用' : '待连接'}</strong></div></section>

      <section className="wide-card glass-card">
        <div className="section-title"><div><p>最近短信</p><span>会话与验证码本地识别</span></div><button onClick={() => onNavigate('messages')}>全部</button></div>
        <div className="recent-list">
          {recent.map((message) => (
            <div className="recent-row" key={message.id}>
              <div className={`message-icon category-${message.category}`}><MessageSquareText size={17} /></div>
              <div className="grow"><strong>{message.sender}</strong><span>{message.body}</span></div>
              {message.code ? <code>{message.code}</code> : null}
              <time>{timeLabel(message.receivedAt)}</time>
            </div>
          ))}
        </div>
      </section>

      <section className="renew-card glass-card">
        <div className="section-title"><div><p>最近续费</p><span>7 天内会主动提醒</span></div></div>
        {nextRenewal ? (
          <>
            <div className="flag-orb">{flagEmoji(nextRenewal.countryCode)}</div>
            <h3>{nextRenewal.name}</h3>
            <p>{nextRenewal.operator}</p>
            <strong className="deadline">{dateDistance(nextRenewal.renewalDate ?? nextRenewal.expiryDate)}</strong>
            <button className="secondary full" onClick={() => onNavigate('esim')}>查看档案</button>
          </>
        ) : <p className="muted">暂无期限提醒</p>}
      </section>
    </div>
  );
}

function Messages({ snapshot, onSnapshot }: { snapshot: AppSnapshot; onSnapshot: (value: AppSnapshot) => void }) {
  const [query, setQuery] = useState('');
  const [archived, setArchived] = useState(false);
  const [selectedSender, setSelectedSender] = useState<string | null>(() => snapshot.messages[0]?.sender ?? null);
  const filtered = snapshot.messages.filter((message) => message.archived === archived && `${message.sender} ${message.body}`.toLowerCase().includes(query.toLowerCase()));
  const senders = [...new Set(filtered.map((message) => message.sender))];
  const timeline = filtered.filter((message) => message.sender === selectedSender);

  const selectSender = async (sender: string) => {
    setSelectedSender(sender);
    onSnapshot(await markSenderRead(sender));
  };

  return (
    <section className="messages-layout glass-heavy">
      <div className="conversation-list">
        <div className="search-box"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索号码或短信" /></div>
        <div className="segmented"><button className={!archived ? 'active' : ''} onClick={() => setArchived(false)}>收件箱</button><button className={archived ? 'active' : ''} onClick={() => setArchived(true)}>归档</button></div>
        <div className="conversation-scroll">
          {senders.map((sender) => {
            const messages = filtered.filter((message) => message.sender === sender);
            const latest = messages[0];
            const unread = messages.filter((message) => message.unread).length;
            return (
              <button key={sender} className={selectedSender === sender ? 'conversation active' : 'conversation'} onClick={() => selectSender(sender)}>
                <div className="avatar">{sender.slice(-2)}</div>
                <div className="grow"><div><strong>{sender}</strong><time>{timeLabel(latest.receivedAt)}</time></div><span>{latest.body}</span></div>
                {unread ? <b>{unread}</b> : null}
              </button>
            );
          })}
        </div>
      </div>
      <div className="timeline">
        <div className="timeline-header"><div><strong>{selectedSender ?? '选择会话'}</strong><span>{timeline.length} 条本地短信</span></div><span className="pill"><Database size={13} />本地存储</span></div>
        <div className="timeline-scroll">
          {timeline.map((message) => (
            <article className={`sms-bubble category-${message.category}`} key={message.id}>
              <div className="bubble-meta"><span>{categoryLabel(message)}</span><time>{new Date(message.receivedAt).toLocaleString()}</time></div>
              <p>{message.body}</p>
              {message.code ? <button className="code-card" onClick={() => navigator.clipboard.writeText(message.code!)}><span>验证码</span><strong>{message.code}</strong><Copy size={15} /></button> : null}
              <button className="bubble-action" onClick={async () => onSnapshot(await archiveMessage(message.id, !message.archived))}><Archive size={14} />{message.archived ? '恢复' : '归档'}</button>
            </article>
          ))}
          {!timeline.length ? <div className="empty-state">选择左侧会话查看短信</div> : null}
        </div>
      </div>
    </section>
  );
}

function EsimPage({ snapshot, onSnapshot }: { snapshot: AppSnapshot; onSnapshot: (value: AppSnapshot) => void }) {
  const [query, setQuery] = useState('');
  const [activation, setActivation] = useState('');
  const [result, setResult] = useState('');
  const profiles = snapshot.esimProfiles.filter((profile) => `${profile.name} ${profile.operator} ${profile.tags.join(' ')}`.toLowerCase().includes(query.toLowerCase()));

  const addProfile = async () => {
    const now = new Date().toISOString();
    const profile: EsimProfile = {
      id: crypto.randomUUID(),
      name: '新 eSIM',
      countryCode: 'CN',
      operator: '待填写运营商',
      autoRenew: false,
      status: 'standby',
      tags: ['新建'],
      updatedAt: now,
    };
    onSnapshot(await upsertEsimProfile(profile));
  };

  const install = async () => {
    if (!activation.trim()) return;
    try { setResult(await installEsimActivationCode(activation.trim())); }
    catch (error) { setResult(error instanceof Error ? error.message : String(error)); }
  };

  return (
    <div className="esim-page">
      <section className="esim-toolbar glass-card">
        <div className="search-box grow"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索名称、运营商或标签" /></div>
        <button className="secondary" onClick={addProfile}><Plus size={16} />新建档案</button>
      </section>

      <section className="install-strip glass-card">
        <div className="install-icon"><Smartphone /></div>
        <div className="grow"><strong>安装 eSIM</strong><span>优先交给 Windows 原生 LPA；只有真实 eUICC 才开放实验桥接。</span></div>
        <input value={activation} onChange={(event) => setActivation(event.target.value)} placeholder="LPA: 或 1$ activation code" />
        <button className="primary" onClick={install}>交给系统安装</button>
        {result ? <small className="install-result">{result}</small> : null}
      </section>

      <div className="wallet-grid">
        {profiles.map((profile) => (
          <article className={`wallet-card glass-heavy status-${profile.status}`} key={profile.id}>
            <div className="wallet-top"><div className="country-flag">{flagEmoji(profile.countryCode)}</div><span className={`pill status-${profile.status}`}>{profile.status === 'active' ? '使用中' : profile.status === 'standby' ? '备用' : '已过期'}</span></div>
            <h3>{profile.name}</h3><p>{profile.operator}</p>
            <div className="wallet-number">{profile.phoneNumber ?? profile.plan ?? '未填写号码 / 套餐'}</div>
            <div className="wallet-facts">
              <div><span>保号 / 续费</span><strong>{profile.keepAliveCost != null ? `${profile.keepAliveCost} ${profile.currency ?? ''}` : '未设置'}</strong></div>
              <div><span>下个期限</span><strong>{dateDistance(profile.renewalDate ?? profile.expiryDate)}</strong></div>
            </div>
            <div className="tag-row">{profile.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>
            <div className="wallet-actions"><button className="secondary"><Settings2 size={14} />编辑</button><button className="danger-icon" onClick={async () => onSnapshot(await deleteEsimProfile(profile.id))}><Trash2 size={15} /></button></div>
          </article>
        ))}
      </div>
    </div>
  );
}

function Devices({ snapshot, onSnapshot }: { snapshot: AppSnapshot; onSnapshot: (value: AppSnapshot) => void }) {
  const [refreshing, setRefreshing] = useState(false);
  const refresh = async () => {
    setRefreshing(true);
    try { onSnapshot(await refreshDevices()); } finally { setRefreshing(false); }
  };
  return (
    <div className="devices-page">
      <div className="section-heading"><div><h2>蜂窝设备与能力</h2><p>Windows Native 优先，AT 只作为 SMS 兼容层，不伪造 eUICC。</p></div><button className="secondary" onClick={refresh}><RefreshCw size={16} className={refreshing ? 'spin' : ''} />重新扫描</button></div>
      <div className="device-grid">
        {snapshot.devices.map((device) => (
          <article className="device-card glass-card" key={device.id}>
            <div className="device-icon"><Gauge /></div>
            <div className="grow"><div className="device-title"><h3>{device.label}</h3><span className={`status-dot ${device.status}`} /></div><p>{device.model ?? device.kind}</p><strong>{device.operator ?? '未注册网络'}</strong></div>
            <div className="device-signal"><Signal size={16} /><strong>{device.signal ?? 0}%</strong><span>{device.networkClass ?? '—'}</span></div>
            <div className="cap-grid">
              <span className={device.capabilities.smsReceive ? 'ok' : ''}>SMS 接收</span>
              <span className={device.capabilities.smsSend ? 'ok' : ''}>SMS 发送</span>
              <span className={device.capabilities.nativeEsim ? 'ok' : ''}>Windows LPA</span>
              <span className={device.capabilities.euiccBridge ? 'ok' : ''}>eUICC Bridge</span>
            </div>
          </article>
        ))}
        {!snapshot.devices.length ? <div className="empty-state glass-card">没有发现蜂窝设备。仍可管理已有 eSIM 档案，但无法凭空接收短信或安装 eSIM。</div> : null}
      </div>
    </div>
  );
}

function Settings({ appearance, onChange, snapshot }: { appearance: Appearance; onChange: (value: Appearance) => void; snapshot: AppSnapshot }) {
  return (
    <div className="settings-grid">
      <section className="settings-card glass-card">
        <div className="setting-title"><CircleGauge /><div><h3>Liquid Glass</h3><p>完整模式与右下角迷你面板共享外观参数。</p></div></div>
        <label className="slider-row"><span><strong>玻璃清晰度</strong><small>{appearance.glass}%</small></span><input type="range" min="20" max="100" value={appearance.glass} onChange={(event) => onChange({ ...appearance, glass: Number(event.target.value) })} /></label>
        <label className="slider-row"><span><strong>环境染色</strong><small>{appearance.tint}%</small></span><input type="range" min="0" max="80" value={appearance.tint} onChange={(event) => onChange({ ...appearance, tint: Number(event.target.value) })} /></label>
        <Toggle label="界面动效与轻量折射" value={appearance.motion} onChange={(value) => onChange({ ...appearance, motion: value })} />
        <Toggle label="语义彩色文字" value={appearance.semantic} onChange={(value) => onChange({ ...appearance, semantic: value })} />
        <div className="accent-row"><span>主强调色</span><div>{(['azure', 'violet', 'mint', 'rose'] as Accent[]).map((accent) => <button key={accent} aria-label={accent} className={`accent-dot ${accent} ${appearance.accent === accent ? 'active' : ''}`} onClick={() => onChange({ ...appearance, accent })} />)}</div></div>
      </section>

      <section className="settings-card glass-card">
        <div className="setting-title"><ShieldCheck /><div><h3>能力与边界</h3><p>安装能力只来自真实 Windows LPA 或真实 eUICC。</p></div></div>
        <StatusLine label="Windows 原生 LPA" value={snapshot.runtime.nativeLpaAvailable ? '可用' : '不可用'} ok={snapshot.runtime.nativeLpaAvailable} />
        <StatusLine label="实验 eUICC Bridge" value={snapshot.runtime.euiccBridgeAvailable ? '可用' : '未发现'} ok={snapshot.runtime.euiccBridgeAvailable} />
        <StatusLine label="lpac" value={snapshot.runtime.lpacFound ? '已发现' : '未发现'} ok={snapshot.runtime.lpacFound} />
        <StatusLine label="平台" value={snapshot.runtime.platform} ok />
      </section>
    </div>
  );
}

function Toggle({ label, value, onChange }: { label: string; value: boolean; onChange: (value: boolean) => void }) {
  return <label className="toggle-row"><span>{label}</span><button className={value ? 'toggle active' : 'toggle'} onClick={() => onChange(!value)}><i /></button></label>;
}

function StatusLine({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return <div className="status-line"><span>{label}</span><strong className={ok ? 'ok-text' : ''}>{value}</strong></div>;
}

function MiniPanel({ snapshot, onExpand }: { snapshot: AppSnapshot; onExpand: () => void }) {
  const device = snapshot.devices.find((item) => item.id === snapshot.selectedDeviceId) ?? snapshot.devices[0];
  const message = snapshot.messages.find((item) => !item.archived);
  const renewal = useMemo(() => snapshot.esimProfiles.find((item) => item.renewalDate || item.expiryDate), [snapshot.esimProfiles]);
  return (
    <div className="mini-shell glass-heavy">
      <header><div className="brand-mark small"><Signal size={16} /></div><div className="grow"><strong>CellularHub</strong><span>迷你面板</span></div><button className="icon-button" onClick={onExpand}><PanelRightOpen size={17} /></button></header>
      <section className="mini-network glass-soft"><div><span className="pill success">{device?.networkClass ?? '蜂窝'}</span><h2>{device?.operator ?? '未连接'}</h2><p>{device?.model ?? '等待设备'}</p></div><div className="mini-signal"><strong>{device?.signal ?? 0}</strong><span>%</span></div></section>
      <section className="mini-message glass-soft"><div className={`message-icon category-${message?.category ?? 'notice'}`}><MessageSquareText size={16} /></div><div className="grow"><div><strong>{message?.sender ?? '暂无短信'}</strong><time>{message ? timeLabel(message.receivedAt) : ''}</time></div><p>{message?.body ?? '收到的新短信会显示在这里。'}</p>{message?.code ? <button onClick={() => navigator.clipboard.writeText(message.code!)}><span>验证码</span><strong>{message.code}</strong><Copy size={13} /></button> : null}</div></section>
      <section className="mini-renewal glass-soft"><CreditCard size={18} /><div className="grow"><span>最近续费 / 到期</span><strong>{renewal?.name ?? '暂无提醒'}</strong></div><b>{renewal ? dateDistance(renewal.renewalDate ?? renewal.expiryDate) : '—'}</b></section>
      <footer><ShieldCheck size={14} />短信与档案保留在本机</footer>
    </div>
  );
}
