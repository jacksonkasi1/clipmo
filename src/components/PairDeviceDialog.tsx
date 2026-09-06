// ** import types
import type { Settings } from '../lib/types';

// ** import utils
import { mutationErrorMessage } from '../lib/mutation-error';

// ** import lib
import { useEffect, useRef, useState } from 'react';
import { CheckCircle2, Link2, MonitorUp, RefreshCw, Wifi, X } from 'lucide-react';

import { useStore } from '../lib/store';
import QRCode from 'qrcode';
import { version } from '../../package.json';

interface PairDeviceDialogProps {
  open: boolean;
  onClose: () => void;
  onSettingsUpdated?: (updated: Partial<Settings>) => void;
}

const PAIRING_CODE_LENGTH = 6;

export function PairDeviceDialog({ open, onClose, onSettingsUpdated }: PairDeviceDialogProps) {
  const settings = useStore((state) => state.settings);
  const sync = useStore((state) => state.sync);
  const closePairing = useStore((state) => state.closePairing);
  const connectDevice = useStore((state) => state.joinDevice);
  const forgetDevice = useStore((state) => state.forgetSyncDevice);
  const loadSyncState = useStore((state) => state.loadSyncState);
  const regeneratePairingCode = useStore((state) => state.regeneratePairingCode);
  const [joinCode, setJoinCode] = useState('');
  const [joinAddress, setJoinAddress] = useState('');
  const [busy, setBusy] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [notice, setNotice] = useState('');
  const [now, setNow] = useState(Date.now());
  const [qr, setQr] = useState('');
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [onClose, open]);

  useEffect(() => {
    if (!open) {
      setJoinCode('');
      setJoinAddress('');
      setError(null);
    }
  }, [open]);

  const pairingCode = sync?.pairingCode ?? settings?.syncPairingCode ?? '';
  const pairingActive = Boolean(sync?.enabled && (sync.pairingUntil ?? 0) > now);
  const invite = `clipmo://pair?v=3&device=${encodeURIComponent(sync?.device.id ?? '')}&code=${pairingCode}${sync?.localAddress ? `&address=${encodeURIComponent(sync.localAddress)}` : ''}`;
  useEffect(() => {
    if (!open) return;
    const timer = window.setInterval(() => {
      setNow(Date.now());
      void loadSyncState().catch(() => {});
    }, 1000);
    return () => window.clearInterval(timer);
  }, [open, loadSyncState]);
  useEffect(() => {
    let active = true;
    setQr('');
    if (open && pairingActive) {
      void QRCode.toDataURL(invite, { width: 140, margin: 4, errorCorrectionLevel: 'M' })
        .then((url) => { if (active) setQr(url); })
        .catch(() => { if (active) setError('QR code unavailable. Use the six-digit code.'); });
    }
    return () => { active = false; };
  }, [open, pairingActive, invite]);

  if (!open || !settings) return null;

  const peers = sync?.peers ?? [];
  const normalizedJoinCode = joinCode.replace(/\D/g, '').slice(0, PAIRING_CODE_LENGTH);
  const canJoin = normalizedJoinCode.length === PAIRING_CODE_LENGTH;

  const disablePairing = async () => {
    setBusy(true);
    setError(null);
    try { await closePairing(); }
    catch (reason) { setError(mutationErrorMessage('Pairing could not be closed.', reason)); }
    finally { setBusy(false); }
  };

  const joinDevice = async () => {
    if (busy) return;
    if (!canJoin) {
      setError('Enter the six-digit code shown on the other device.');
      inputRef.current?.focus();
      return;
    }
    setBusy(true);
    setError(null);
    setConnecting(true);
    setNotice('');
    try {
      if (joinAddress.trim()) await connectDevice(normalizedJoinCode, undefined, joinAddress.trim());
      else await connectDevice(normalizedJoinCode);
      setNotice('Device connected. New clipboard items sync automatically.');
      setJoinCode('');
    } catch (saveError) {
      setError(mutationErrorMessage('This device could not join the pairing.', saveError));
    } finally {
      setConnecting(false);
      setBusy(false);
    }
  };

  const replaceCode = async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await regeneratePairingCode();
      if (next) onSettingsUpdated?.(next);
    } catch (regenerateError) {
      setError(mutationErrorMessage('A new pairing code could not be created.', regenerateError));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pair-device-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section
        className="pair-device-dialog"
        role="dialog"
        aria-modal="true"
        aria-busy={busy}
        aria-labelledby="pair-device-title"
      >
        <header>
          <span className="pair-device-heading-icon"><MonitorUp size={19} aria-hidden /></span>
          <div>
            <h2 id="pair-device-title">Add another device</h2>
            <p>Clipmo {version} · Connect devices on the same local network.</p>
          </div>
          <button type="button" className="icon-button" aria-label="Close add device" onClick={onClose}>
            <X size={16} aria-hidden />
          </button>
        </header>

        <div className="pair-device-content">
          {sync?.compatibilityWarning && <p role="status">{sync.compatibilityWarning}</p>}
          <section className={`pair-device-step${pairingActive && qr ? ' has-qr' : ''}`} aria-labelledby="share-code-title">
            <div className="pair-device-step-heading">
              <Wifi size={17} aria-hidden />
              <div>
                <h3 id="share-code-title">Connect another device to this one</h3>
                <p>On the other device, choose Add device and enter this code.</p>
              </div>
            </div>
            <div className="pair-device-code-row">
              <output className="pair-device-code" aria-label={pairingActive ? `Pairing code ${pairingCode}` : 'Pairing closed'}>
                {pairingActive ? pairingCode : '------'}
              </output>
              <button
                type="button"
                className="secondary-button"
                disabled={busy}
                title="Generate a new invitation; saved connections stay connected"
                onClick={() => void replaceCode()}
              >
                <RefreshCw size={14} aria-hidden /> New code
              </button>
            </div>
            {pairingActive && qr && <figure className="pair-device-qr"><img src={qr} width={140} height={140} alt="Scan this pairing invitation in Clipmo for Android" /><figcaption>Android: Devices → Scan QR code</figcaption></figure>}
            {pairingActive ? (
              <div className="pair-device-actions-row">
                <span className="pair-device-status-badge">
                  <span className="pair-device-status-dot" /> Pairing open · {Math.max(0, Math.ceil(((sync?.pairingUntil ?? 0) - now) / 1000))}s
                </span>
                <button type="button" className="secondary-button" disabled={busy} onClick={() => void disablePairing()}>
                  <X size={14} aria-hidden /> Close pairing
                </button>
              </div>
            ) : (
              <button type="button" className="primary-button" disabled={busy} onClick={() => void replaceCode()}>
                <Wifi size={15} aria-hidden /> Start pairing
              </button>
            )}
          </section>

          <div className="pair-device-separator"><span>or</span></div>

          <section className="pair-device-step" aria-labelledby="join-code-title">
            <div className="pair-device-step-heading">
              <Link2 size={17} aria-hidden />
              <div>
                <h3 id="join-code-title">Join a device that already shows a code</h3>
                <p>Enter the six-digit code shown on the other device.</p>
              </div>
            </div>
            <details className="pair-device-help" onToggle={(event) => { if (!event.currentTarget.open) setJoinAddress(''); }}>
              <summary>Connection help</summary>
              {sync?.localAddress && <p>This device: <code>{sync.localAddress}</code></p>}
              <label>Other device’s address
              <input value={joinAddress} onChange={(event) => setJoinAddress(event.target.value)} placeholder="192.168.1.4:47634" aria-label="Other device LAN address" />
              </label>
              <p>Use this only if code entry cannot find the device. Both apps need version 0.2.12 or newer.</p>
            </details>
            <form className="pair-device-join" onSubmit={(event) => {
              event.preventDefault();
              void joinDevice();
            }}>
              <input
                ref={inputRef}
                type="text"
                className="pair-device-code-input"
                value={joinCode}
                inputMode="numeric"
                autoComplete="one-time-code"
                maxLength={PAIRING_CODE_LENGTH}
                placeholder="000000"
                aria-label="Pairing code from another device"
                onChange={(event) => {
                  setJoinCode(event.target.value.replace(/\D/g, '').slice(0, PAIRING_CODE_LENGTH));
                  setError(null);
                }}
              />
              <button type="submit" className="primary-button" disabled={busy || !canJoin}>
                <Link2 size={15} aria-hidden /> {connecting ? 'Connecting…' : 'Connect'}
              </button>
            </form>
          </section>

          <p>New codes keep saved devices connected.</p>
          {peers.length > 0 && (
            <ul className="pair-device-peers" aria-label="Saved connections">
              {peers.map((peer) => (
                <li key={peer.device.id}>
                  <span>{peer.device.name} · {sync?.enabled && peer.status === 'synced' ? 'Connected' : 'Offline'}</span>
                  <button type="button" className="secondary-button" disabled={busy}
                    aria-label={`Remove ${peer.device.name}`} onClick={() => {
                      setBusy(true);
                      setError(null);
                      void forgetDevice(peer.device.id)
                        .catch((reason) => setError(mutationErrorMessage('Device could not be removed.', reason)))
                        .finally(() => setBusy(false));
                    }}>Remove</button>
                </li>
              ))}
            </ul>
          )}
          {connecting && <p role="status">Connecting… Keep pairing open on the other device.</p>}
          {notice && <p role="status"><CheckCircle2 size={16} aria-hidden /> {notice}</p>}
          {pairingActive && peers.length === 0 && !connecting && (
            <p className="pair-device-waiting" aria-live="polite">Ready to pair. Enter this code on another device or scan the QR code in Clipmo for Android.</p>
          )}
          {error && <p className="pair-device-error" role="alert">{error}</p>}
        </div>
      </section>
    </div>
  );
}
