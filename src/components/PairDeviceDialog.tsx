// ** import types
import type { Settings } from '../lib/types';

// ** import utils
import { mutationErrorMessage } from '../lib/mutation-error';

// ** import lib
import { useEffect, useRef, useState } from 'react';
import { CheckCircle2, Link2, MonitorUp, RefreshCw, Wifi, X } from 'lucide-react';

import { useStore } from '../lib/store';

interface PairDeviceDialogProps {
  open: boolean;
  onClose: () => void;
  onSettingsUpdated?: (updated: Partial<Settings>) => void;
}

const PAIRING_CODE_LENGTH = 6;

export function PairDeviceDialog({ open, onClose, onSettingsUpdated }: PairDeviceDialogProps) {
  const settings = useStore((state) => state.settings);
  const sync = useStore((state) => state.sync);
  const saveSettings = useStore((state) => state.saveSettings);
  const regeneratePairingCode = useStore((state) => state.regeneratePairingCode);
  const [joinCode, setJoinCode] = useState('');
  const [busy, setBusy] = useState(false);
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
      setError(null);
    }
  }, [open]);

  if (!open || !settings) return null;

  const pairingCode = sync?.pairingCode ?? settings.syncPairingCode;
  const peers = sync?.peers ?? [];
  const normalizedJoinCode = joinCode.replace(/\D/g, '').slice(0, PAIRING_CODE_LENGTH);
  const canJoin = normalizedJoinCode.length === PAIRING_CODE_LENGTH;

  const enablePairing = async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await saveSettings({ ...settings, syncEnabled: true });
      onSettingsUpdated?.(next);
    } catch (saveError) {
      setError(mutationErrorMessage('Pairing could not be started.', saveError));
    } finally {
      setBusy(false);
    }
  };

  const disablePairing = async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await saveSettings({ ...settings, syncEnabled: false });
      onSettingsUpdated?.(next);
    } catch (saveError) {
      setError(mutationErrorMessage('Pairing could not be stopped.', saveError));
    } finally {
      setBusy(false);
    }
  };

  const joinDevice = async () => {
    if (!canJoin) {
      setError('Enter the six-digit code shown on the other device.');
      inputRef.current?.focus();
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const next = await saveSettings({
        ...settings,
        syncEnabled: true,
        syncPairingCode: normalizedJoinCode,
      });
      onSettingsUpdated?.(next);
      setJoinCode('');
    } catch (saveError) {
      setError(mutationErrorMessage('This device could not join the pairing.', saveError));
    } finally {
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
        aria-labelledby="pair-device-title"
      >
        <header>
          <span className="pair-device-heading-icon"><MonitorUp size={19} aria-hidden /></span>
          <div>
            <h2 id="pair-device-title">Add another device</h2>
            <p>Connect Clipmo desktops or mobile devices on the same local network.</p>
          </div>
          <button type="button" className="icon-button" aria-label="Close add device" onClick={onClose}>
            <X size={16} aria-hidden />
          </button>
        </header>

        <div className="pair-device-content">
          <section className="pair-device-step" aria-labelledby="share-code-title">
            <div className="pair-device-step-heading">
              <Wifi size={17} aria-hidden />
              <div>
                <h3 id="share-code-title">Connect another device to this one</h3>
                <p>On the other device, choose Add device and enter this code.</p>
              </div>
            </div>
            <div className="pair-device-code-row">
              <output className="pair-device-code" aria-label={`Pairing code ${pairingCode}`}>
                {pairingCode}
              </output>
              <button
                type="button"
                className="secondary-button"
                disabled={busy}
                title="Replace this code on all devices that should remain connected"
                onClick={() => void replaceCode()}
              >
                <RefreshCw size={14} aria-hidden /> New code
              </button>
            </div>
            {settings.syncEnabled ? (
              <div className="pair-device-actions-row">
                <span className="pair-device-status-badge">
                  <span className="pair-device-status-dot" /> Pairing active
                </span>
                <button type="button" className="secondary-button" disabled={busy} onClick={() => void disablePairing()}>
                  <X size={14} aria-hidden /> Close pairing
                </button>
              </div>
            ) : (
              <button type="button" className="primary-button" disabled={busy} onClick={() => void enablePairing()}>
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
                <p>Enter its six-digit code here. Both devices must use the same code.</p>
              </div>
            </div>
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
                <Link2 size={15} aria-hidden /> Connect
              </button>
            </form>
          </section>

          {peers.length > 0 && (
            <div className="pair-device-connected" aria-live="polite">
              <CheckCircle2 size={16} aria-hidden />
              <span>Connected: {peers.map((peer) => peer.device.name).join(', ')}</span>
            </div>
          )}
          {settings.syncEnabled && peers.length === 0 && (
            <p className="pair-device-waiting" aria-live="polite">
              Waiting for another Clipmo device with code {pairingCode}… New clipboard items sync automatically once connected.
            </p>
          )}
          {error && <p className="pair-device-error" role="alert">{error}</p>}
        </div>
      </section>
    </div>
  );
}
