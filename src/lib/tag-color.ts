const TAG_COLOR_COUNT = 6;
const TAG_COLOR_STORAGE_KEY = 'clipmo.tag-colors';

export type TagColorMap = Record<string, string>;

export function tagColorKey(tag: string): string {
  return tag.trim().toLowerCase();
}

/** Stable color assignment: the same normalized tag always gets the same swatch. */
export function tagColorIndex(tag: string): number {
  let hash = 0;
  for (const character of tag.trim().toLowerCase()) {
    hash = ((hash << 5) - hash + character.charCodeAt(0)) | 0;
  }
  return Math.abs(hash) % TAG_COLOR_COUNT;
}

export function tagColorClass(tag: string): string {
  return `tag-color-${tagColorIndex(tag)}`;
}

export function loadTagColors(): TagColorMap {
  if (typeof localStorage === 'undefined') return {};
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(TAG_COLOR_STORAGE_KEY) ?? '{}');
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter((entry): entry is [string, string] => (
        typeof entry[1] === 'string' && /^#[0-9a-f]{6}$/i.test(entry[1])
      )),
    );
  } catch {
    return {};
  }
}

export function saveTagColors(colors: TagColorMap): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(TAG_COLOR_STORAGE_KEY, JSON.stringify(colors));
  } catch {
    // Color customization is optional; a blocked/full web storage must not break filtering.
  }
}
