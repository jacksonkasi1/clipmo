const TAG_COLOR_COUNT = 6;

/** Stable color assignment: the same normalized tag always gets the same swatch. */
export function tagColorClass(tag: string): string {
  let hash = 0;
  for (const character of tag.trim().toLowerCase()) {
    hash = ((hash << 5) - hash + character.charCodeAt(0)) | 0;
  }
  return `tag-color-${Math.abs(hash) % TAG_COLOR_COUNT}`;
}
