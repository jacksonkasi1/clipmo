// ** import lib
import { api } from './tauri';
import { toast } from './toast';
import { mutationErrorMessage } from './mutation-error';

export async function pasteSelectedItems(ids: number[]): Promise<void> {
  try {
    await api.pasteMultipleActive(ids, 'original');
  } catch (error) {
    toast(mutationErrorMessage('Paste stopped.', error), 'error');
  }
}

export async function copySelectedItems(ids: number[]): Promise<void> {
  try {
    await api.copyMultipleToClipboard(ids, 'original');
    toast(`Copied ${ids.length} items to clipboard`, 'info');
  } catch (error) {
    toast(mutationErrorMessage('Copy failed.', error), 'error');
  }
}
