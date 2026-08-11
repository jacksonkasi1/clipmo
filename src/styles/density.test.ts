// ** import lib
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { ROW_HEIGHT } from '../components/ItemList';

const app = readFileSync(fileURLToPath(new URL('./app.css', import.meta.url)), 'utf8');
const tokens = readFileSync(fileURLToPath(new URL('./tokens.css', import.meta.url)), 'utf8');

/** Returns the body of the first rule whose selector list matches exactly. */
function rule(css: string, selector: string): string {
  const start = css.indexOf(`\n${selector} {`);
  expect(start, `missing rule for ${selector}`).toBeGreaterThan(-1);
  const open = css.indexOf('{', start);
  return css.slice(open + 1, css.indexOf('}', open));
}

describe('quiet icon actions', () => {
  it('never paints a filled accent block behind an active toolbar icon', () => {
    const active = rule(app, '.icon-button.is-active');

    // The whole point of the pass: state is a tinted glyph, not a blue button.
    expect(active).toContain('color: var(--accent)');
    expect(active).toContain('background: transparent');
    expect(active).not.toContain('accent-soft');
  });

  it('defaults to a transparent, borderless icon with a neutral hover only', () => {
    const base = rule(app, '.icon-button');
    const hover = rule(app, '.icon-button:hover:not(:disabled)');

    expect(base).toContain('background: transparent');
    expect(base).toContain('border: 0');
    // Hover uses the neutral row wash, not `--hover`/`--accent-soft` surfaces.
    expect(hover).toContain('background: var(--row-hover)');
  });
});

describe('integrated search header', () => {
  it('has no inner search-field card left in the stylesheet', () => {
    expect(app).not.toContain('.search-field');
  });

  it('renders the input straight onto the header surface', () => {
    const input = rule(app, '.search-header input');

    expect(input).toContain('background: transparent');
    expect(input).toContain('border: 0');
    expect(rule(app, '.search-header')).toContain('cursor: text');
  });
});

describe('list density', () => {
  it('drives row height from the value the virtualizer measures', () => {
    // A hard-coded height here would silently desynchronise from
    // `ROW_HEIGHT` and leave gaps or overlaps between virtualised rows.
    expect(rule(app, '.item-row')).toContain('height: var(--row-height, 40px)');
    expect(ROW_HEIGHT.quick).toBe(32);
    expect(ROW_HEIGHT.full).toBe(40);
  });

  it('keeps every chrome height on the shared compact scale', () => {
    expect(tokens).toContain('--header-h: 44px');
    expect(tokens).toContain('--footer-h: 30px');
    expect(rule(app, '.history-content'))
      .toContain('grid-template-rows: var(--header-h) minmax(0, 1fr) var(--footer-h)');
  });

  it('shows meaningfully more rows than the previous 46px/48px/46px layout', () => {
    const before = 620 - 48 - 46; // old chrome, quick flyout height
    const after = 620 - 42 - 28; // new quick chrome
    expect(Math.floor(after / ROW_HEIGHT.quick)).toBeGreaterThan(Math.floor(before / 46) + 5);
  });
});

describe('row trailing strip', () => {
  it('reserves a fixed footprint so the favorite star cannot reflow the title', () => {
    // The favorite button is `opacity: 0` until hover, but the trailing
    // cluster must still occupy the same width on every row — otherwise
    // the title's right edge would dance as the star faded in and the
    // grid would visibly shift between rows. Width + flex-basis + min-width
    // together pin the strip.
    const trailing = rule(app, '.row-trailing');
    expect(trailing).toContain('width: 44px');
    expect(trailing).toContain('min-width: 44px');
    expect(trailing).toContain('flex: 0 0 44px');
    expect(trailing).toContain('justify-content: flex-end');
  });
});
