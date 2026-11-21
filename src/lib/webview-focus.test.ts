import { afterEach, describe, expect, it, vi } from 'vitest';

const setFocus = vi.fn().mockResolvedValue(undefined);
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ setFocus }) }));

import { focusNativeWebview } from './webview-focus';

describe('focusNativeWebview', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    setFocus.mockClear();
  });

  it('hands native focus to the webview on macOS', async () => {
    vi.stubGlobal('navigator', { platform: 'MacIntel', userAgent: 'Macintosh' });
    await focusNativeWebview();
    expect(setFocus).toHaveBeenCalledTimes(1);
  });

  it('does nothing on Windows, where it would make the quick window lose focus and hide', async () => {
    vi.stubGlobal('navigator', { platform: 'Win32', userAgent: 'Windows NT 10.0' });
    await focusNativeWebview();
    expect(setFocus).not.toHaveBeenCalled();
  });
});
