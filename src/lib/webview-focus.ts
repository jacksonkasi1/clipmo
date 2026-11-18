// ** import lib
import { getCurrentWebview } from '@tauri-apps/api/webview';

/**
 * Hands native keyboard focus to this window's webview.
 *
 * `window.focus()` / `input.focus()` only move *DOM* focus. On macOS a warm,
 * re-shown WKWebView can be the key window without being the first responder,
 * in which case no keydown (arrows, Enter, Esc) is delivered to the page until
 * the user clicks. Best-effort: outside Tauri (tests, browser dev) or when the
 * capability is missing this silently does nothing.
 */
export async function focusNativeWebview(): Promise<void> {
  try {
    await getCurrentWebview().setFocus();
  } catch {
    // Not running inside Tauri, or the webview is already focused.
  }
}
