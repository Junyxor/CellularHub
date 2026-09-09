import { useEffect, useMemo, useState } from 'react';
import {
  connectAt,
  connectWindowsMbn,
  getSnapshot,
  listDevices,
  listMessages,
  probeEsimCompat,
  runReleaseDiagnostics,
} from './lib/api';
import type {
  CellularSnapshot,
  DeviceInfo,
  EsimCompatReport,
  ReleaseDiagnostics,
  SmsMessage,
} from './lib/types';

type Page = 'overview' | 'devices' | 'sms' | 'esim' | 'diagnostics';

const pages: Array<{ id: Page; label: string }> = [
  { id: 'overview', label: '概览' },
  { id: 'devices', label: '设备' },
  { id: 'sms', label: '短信' },
  { id: 'esim', label: 'eSIM' },
  { id: 'diagnostics', label: '诊断' },
];

function fmtTime(value?: string) {
  if (!value) return '—';
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? value : d.toLocaleString();
}

export default function App() {
  const [page, setPage] = useState<Page>('overview');
  const [snapshot, setSnapshot] = useState<CellularSnapshot | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [messages, setMessages] = useState<SmsMessage[]>([]);
  const [esim, setEsim] = useState<EsimCompatReport | null>(null);
  const [diag, setDiag] = useState<ReleaseDiagnostics | null>(null);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');

  const unread = useMemo(() => messages.filter((m) => m.unread && !m.archived).length, [messages]);

  async function refreshCore() {
    setError('');
    try {
      const [s, d, m] = await Promise.all([getSnapshot(), listDevices(), listMessages()]);
      setSnapshot(s); setDevices(d); setMessages(m);
    } catch (e) { setError(String(e)); }
  }

  useEffect(() => { void refreshCore(); }, []);

  async function connect(device: DeviceInfo) {
    setBusy(device.id); setError('');
    try {
      if (device.backend === 'windows_mbn') await connectWindowsMbn(device.id);
      else if (device.backend === 'at' && device.port) await connectAt({ port: device.port, baudRate: 115200 });
      await refreshCore();
      setPage('overview');
    } catch (e) { setError(String(e)); }
    finally { setBusy(''); }
  }

  async function scanEsim() {
    setBusy('esim'); setError('');
    try { setEsim(await probeEsimCompat()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(''); }
  }

  async function diagnostics() {
    setBusy('diag'); setError('');
    try { setDiag(await runReleaseDiagnostics()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(''); }
  }

  return (
    <div className="shell">
      <aside className="sidebar glass">
        <div className="brand"><span className="brand-orb"/><div><strong>CellularHub</strong><small>1.0 · SMOKE RC</small></div></div>
        <nav>{pages.map((item) => <button key={item.id} className={page === item.id ? 'nav active' : 'nav'} onClick={() => setPage(item.id)}>{item.label}{item.id === 'sms' && unread > 0 ? <b>{unread}</b> : null}</button>)}</nav>
        <div className="sidebar-foot"><span className={snapshot?.connected ? 'dot on' : 'dot'} />{snapshot?.connected ? '蜂窝后端已连接' : '等待蜂窝设备'}</div>
      </aside>

      <main className="content">
        <header><div><p className="eyebrow">WINDOWS CELLULAR CONTROL CENTER</p><h1>{pages.find((p) => p.id === page)?.label}</h1></div><button className="ghost" onClick={() => void refreshCore()}>刷新</button></header>
        {error ? <div className="error glass"><strong>当前操作失败</strong><span>{error}</span></div> : null}

        {page === 'overview' && <>
          <section className="hero glass">
            <div><p className="eyebrow">CURRENT LINE</p><h2>{snapshot?.operatorName || '尚未连接'}</h2><p>{snapshot?.deviceName || '从“设备”页面选择 Windows MBN 或 AT Modem'}</p></div>
            <div className="signal"><strong>{snapshot?.signalPercent ?? 0}%</strong><span>{snapshot?.networkType || 'Cellular'}</span></div>
          </section>
          <section className="grid3">
            <article className="card glass"><span>SMS</span><strong className="accent-purple">{snapshot?.smsReady ? '可接收' : '未就绪'}</strong><small>{messages.length} 条本地短信 · {unread} 未读</small></article>
            <article className="card glass"><span>Provider</span><strong className="accent-blue">{snapshot?.backend || 'none'}</strong><small>Windows MBN 优先，AT 兼容兜底</small></article>
            <article className="card glass"><span>eSIM</span><strong className="accent-pink">{snapshot?.esimReady ? '设备可用' : '独立探测'}</strong><small>Windows LPA / 外置真实 eUICC</small></article>
          </section>
          <section className="panel glass"><h3>第一轮试机建议</h3><p>先打开“设备”，优先连接标记为 Windows Native 的 MBN Provider；给当前 SIM/eSIM 发一条真实短信，再到“短信”页面刷新。MBN 不可用时再尝试 AT COM 口。</p></section>
        </>}

        {page === 'devices' && <section className="stack">
          {devices.length === 0 ? <div className="panel glass">没有发现候选蜂窝设备。确认 Modem 驱动已安装，并在 Windows 设备管理器中可见。</div> : devices.map((d) => <article className="device glass" key={d.id}>
            <div><p className="eyebrow">{d.backend === 'windows_mbn' ? 'WINDOWS NATIVE' : 'SMS FALLBACK'}</p><h3>{d.name}</h3><p>{d.manufacturer || 'Unknown vendor'} · {d.product || d.port || d.id}</p><div className="chips"><span>{d.capabilities.smsReceive ? 'SMS Receive' : 'No SMS'}</span>{d.capabilities.signal && <span>Signal</span>}{d.port && <span>{d.port}</span>}</div></div>
            <button disabled={busy === d.id || !d.capabilities.smsReceive} onClick={() => void connect(d)}>{busy === d.id ? '连接中…' : d.backend === 'windows_mbn' ? '连接原生 MBN' : '连接 AT 兼容'}</button>
          </article>)}
        </section>}

        {page === 'sms' && <section className="stack">
          <div className="panel glass row"><div><h3>短信收件箱</h3><p>当前 {messages.length} 条，{unread} 条未读。Smoke RC 重点验证接收链路。</p></div><button onClick={() => void refreshCore()}>刷新短信</button></div>
          {messages.length === 0 ? <div className="panel glass">还没有短信。连接 Provider 后给这张卡发一条验证码或普通短信。</div> : messages.slice().reverse().map((m) => <article className="message glass" key={m.id}><div className="message-head"><strong>{m.sender}</strong><small>{fmtTime(m.receivedAt)}</small></div><p>{m.body}</p><div className="chips"><span>{m.backend}</span>{m.unread && !m.archived ? <span className="purple">未读</span> : null}</div></article>)}
        </section>}

        {page === 'esim' && <section className="stack">
          <div className="panel glass row"><div><h3>eSIM / eUICC 探测</h3><p>不会伪造 eSIM 能力；只有真实 APDU/eUICC 通道才会开放实验兼容。</p></div><button disabled={busy === 'esim'} onClick={() => void scanEsim()}>{busy === 'esim' ? '扫描中…' : '扫描 eUICC'}</button></div>
          {esim && <div className="panel glass"><div className="chips"><span>Windows LPA: {esim.windowsLpaAvailable ? 'Yes' : 'No'}</span><span>lpac: {esim.lpacPath ? 'Found' : 'Not found'}</span></div>{esim.candidates.length ? esim.candidates.map((c) => <div className="candidate" key={c.port}><strong>{c.port}</strong><span>{c.note}</span>{c.eid && <code>EID {c.eid}</code>}</div>) : <p>未检测到真实外置 eUICC/APDU 候选设备。</p>}</div>}
        </section>}

        {page === 'diagnostics' && <section className="stack">
          <div className="panel glass row"><div><h3>1.0 发布健康检查</h3><p>不导出短信正文或 activation code。</p></div><button disabled={busy === 'diag'} onClick={() => void diagnostics()}>{busy === 'diag' ? '检查中…' : '运行自检'}</button></div>
          {diag && <div className="panel glass"><div className="grid3"><div><span>设备</span><strong>{diag.deviceCount}</strong></div><div><span>短信</span><strong>{diag.messageCount}</strong></div><div><span>eSIM 档案</span><strong>{diag.esimProfileCount}</strong></div></div><div className="checks">{diag.checks.map((c) => <div key={c.id}><span className={`dot ${c.status === 'pass' ? 'on' : ''}`}/><strong>{c.label}</strong><p>{c.detail}</p></div>)}</div></div>}
        </section>}
      </main>
    </div>
  );
}
