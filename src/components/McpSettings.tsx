// ** import lib
import { useEffect, useState } from 'react';
import { Copy, Plug } from 'lucide-react';

import { api } from '../lib/tauri';
import { mutationErrorMessage } from '../lib/mutation-error';
import type { McpStatus } from '../lib/types';

/** Ready-to-paste setup for common MCP clients. */
export function mcpSnippets(url: string, token: string): { id: string; label: string; text: string }[] {
  return [
    {
      id: 'claude-code',
      label: 'Claude Code',
      text: `claude mcp add --transport http clipmo ${url} --header "Authorization: Bearer ${token}"`,
    },
    {
      id: 'codex',
      label: 'Codex (~/.codex/config.toml)',
      text: `export CLIPMO_MCP_TOKEN=${token}\n\n[mcp_servers.clipmo]\nurl = "${url}"\nbearer_token_env_var = "CLIPMO_MCP_TOKEN"`,
    },
    {
      id: 'opencode',
      label: 'OpenCode (opencode.json)',
      text: JSON.stringify(
        { $schema: 'https://opencode.ai/config.json', mcp: { clipmo: { type: 'remote', url, headers: { Authorization: `Bearer ${token}` }, enabled: true } } },
        null,
        2,
      ),
    },
    {
      id: 'cursor',
      label: 'Cursor / Windsurf (mcp.json)',
      text: JSON.stringify({ mcpServers: { clipmo: { url, headers: { Authorization: `Bearer ${token}` } } } }, null, 2),
    },
    {
      id: 'claude-desktop',
      label: 'Claude Desktop (claude_desktop_config.json)',
      text: JSON.stringify(
        { mcpServers: { clipmo: { command: 'npx', args: ['-y', 'mcp-remote', url, '--header', `Authorization:Bearer ${token}`] } } },
        null,
        2,
      ),
    },
  ];
}

export function McpSettings() {
  const [status, setStatus] = useState<McpStatus | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api.mcpStatus().then(setStatus).catch(() => undefined);
  }, []);

  const run = async (action: () => Promise<McpStatus>) => {
    setBusy(true);
    setMessage(null);
    try {
      setStatus(await action());
    } catch (error) {
      setMessage(mutationErrorMessage('MCP could not be changed.', error));
    } finally {
      setBusy(false);
    }
  };

  const copy = async (id: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setMessage(`Copied the ${id} setup.`);
    } catch {
      setMessage('Copy failed — select the text and copy it manually.');
    }
  };

  const url = status?.url ?? 'http://127.0.0.1:47650/mcp';
  return (
    <div className="relay-settings">
      <p>
        <Plug size={14} aria-hidden /> Lets Claude, Codex, OpenCode and other MCP apps search, tag, star, organize and
        delete your clips. It listens on this computer only and requires the access token below.
      </p>
      <label>
        <input
          type="checkbox"
          checked={Boolean(status?.enabled)}
          disabled={busy || !status}
          onChange={(event) => void run(() => api.setMcpEnabled(event.target.checked))}
        />{' '}
        Enable local MCP server
      </label>
      {status?.enabled && !status.running && <p role="alert">The server could not start (ports 47650–47659 are busy).</p>}
      {status?.enabled && status.running && (
        <>
          <p>Listening at <code>{url}</code></p>
          {mcpSnippets(url, status.token).map((snippet) => (
            <details key={snippet.id}>
              <summary>{snippet.label}</summary>
              <pre className="mcp-snippet">{snippet.text}</pre>
              <button type="button" className="secondary-button" onClick={() => void copy(snippet.label, snippet.text)}>
                <Copy size={14} aria-hidden /> Copy
              </button>
            </details>
          ))}
          <div className="history-actions">
            <button type="button" className="secondary-button" disabled={busy} onClick={() => void run(() => api.rotateMcpToken())}>
              New access token
            </button>
          </div>
          <p>Treat the token like a password: anything holding it can read and change your clipboard history.</p>
        </>
      )}
      {message && <p role="status">{message}</p>}
    </div>
  );
}
