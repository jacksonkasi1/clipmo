// ** import lib
import { useEffect, useState } from 'react';
import { Cloud } from 'lucide-react';

import { api } from '../lib/tauri';
import { mutationErrorMessage } from '../lib/mutation-error';
import type { RelayBackend, RelayStatus } from '../lib/types';

/**
 * Optional end-to-end-encrypted relay for networks that block direct LAN sync.
 * The mailbox is the user's own (Cloudflare Worker, Firebase, ...); the passphrase
 * never leaves their devices.
 */
export function RelaySettings() {
  const [status, setStatus] = useState<RelayStatus | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [backend, setBackend] = useState<RelayBackend>('endpoint');
  const [url, setUrl] = useState('');
  const [secret, setSecret] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const load = async () => {
    const next = await api.relayStatus();
    setStatus(next);
    setEnabled(next.enabled);
    setBackend(next.backend === 'firebase' ? 'firebase' : 'endpoint');
    setUrl(next.url);
  };

  useEffect(() => {
    void load().catch(() => undefined);
    const timer = window.setInterval(() => {
      void api.relayStatus().then(setStatus).catch(() => undefined);
    }, 5000);
    return () => window.clearInterval(timer);
  }, []);

  const save = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const next = await api.saveRelay({
        enabled,
        backend,
        url,
        secret: secret ? secret : null,
        passphrase: passphrase ? passphrase : null,
      });
      setStatus(next);
      setSecret('');
      setPassphrase('');
      setMessage(enabled ? 'Relay saved. Use the same passphrase on every device.' : 'Relay turned off.');
    } catch (error) {
      setMessage(mutationErrorMessage('Relay could not be saved.', error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="relay-settings">
      <p>
        <Cloud size={14} aria-hidden /> Sync through your own server when devices cannot reach each other directly.
        Everything is encrypted on your devices with your passphrase; text clips, edits, favorites and deletes use the relay.
      </p>
      <label>
        <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /> Enable relay
      </label>
      <label title="Where encrypted clips are parked while your devices are not connected to each other.">
        Backend
        <select value={backend} onChange={(event) => setBackend(event.target.value as RelayBackend)}>
          <option value="endpoint">Cloudflare Worker (or your own HTTP server)</option>
          <option value="firebase">Firebase Realtime Database</option>
        </select>
        <small className="relay-hint">
          {backend === 'firebase'
            ? 'Your own Firebase project. Clipmo talks to its database directly.'
            : 'A small server you deploy once (the included Cloudflare Worker is the easy way). It stores encrypted clips in its own database (D1, Turso or Postgres) — you never enter database details here, only the server address.'}
        </small>
      </label>
      <label title={backend === 'firebase' ? 'Firebase console → Realtime Database → copy the URL at the top.' : 'The https:// address printed after you deploy the Worker (or your server).'}>
        {backend === 'firebase' ? 'Database URL' : 'Server address'}
        <input type="url" value={url} placeholder={backend === 'firebase' ? 'https://your-project-default-rtdb.firebaseio.com' : 'https://clipmo-relay.your-name.workers.dev'} onChange={(event) => setUrl(event.target.value)} />
        <small className="relay-hint">
          {backend === 'firebase'
            ? 'Firebase console → Realtime Database → the URL shown at the top of the data tab.'
            : 'The https:// address shown after you deploy the relay (see relay/README.md).'}
        </small>
      </label>
      <label title={backend === 'firebase' ? 'A Realtime Database secret or auth token so only you can read and write.' : 'The RELAY_TOKEN you set when deploying the Worker. It only lets this app talk to your server.'}>
        {backend === 'firebase' ? 'Database secret / auth token' : 'Access password for your server'}
        <input
          type="password"
          value={secret}
          placeholder={status?.hasSecret ? 'Saved — leave blank to keep' : ''}
          onChange={(event) => setSecret(event.target.value)}
        />
        <small className="relay-hint">
          {backend === 'firebase'
            ? 'Firebase console → Project settings → Service accounts → Database secrets (or an auth token). Keeps strangers out of your database.'
            : 'The value you chose with "wrangler secret put RELAY_TOKEN". Keeps strangers from using your server. Not your encryption key.'}
        </small>
      </label>
      <label title="Used to encrypt your clips on this device before they leave it. Type the exact same passphrase on every device. It is never sent anywhere, so it cannot be recovered if you forget it.">
        Sync passphrase (same on all devices)
        <input
          type="password"
          value={passphrase}
          placeholder={status?.hasPassphrase ? 'Saved — leave blank to keep' : ''}
          onChange={(event) => setPassphrase(event.target.value)}
        />
        <small className="relay-hint">Your secret encryption phrase — choose any long phrase and use the same one on every device. The server only ever stores scrambled text.</small>
      </label>
      <div className="history-actions">
        <button type="button" className="primary-button" disabled={busy} onClick={() => void save()}>Save relay</button>
      </div>
      {status?.enabled && (
        <p role="status">
          {status.lastError
            ? `Relay problem: ${status.lastError}`
            : status.lastOkAt > 0
              ? `Connected · last sync ${new Date(status.lastOkAt).toLocaleTimeString()}`
              : 'Waiting for the first sync…'}
        </p>
      )}
      {message && <p role="status">{message}</p>}
      <p>Setup guide for Cloudflare, Firebase and SQL backends: <code>relay/README.md</code> in the Clipmo repository.</p>
    </div>
  );
}
