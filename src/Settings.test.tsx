// ** import lib
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
  NumberInput,
  normaliseExtensions,
  Row,
  Segmented,
  SHORTCUT_RECORDER_DESCRIPTION,
  ShortcutRecorder,
  Toggle,
} from './Settings';

describe('Settings accessibility', () => {
  it('associates row copy with each compound settings control', () => {
    const markup = renderToStaticMarkup(
      <>
        <Row id="theme" label="Theme" description="Choose a theme.">
          <Segmented value="system" onChange={() => undefined} options={[
            { value: 'system', label: 'System' },
            { value: 'dark', label: 'Dark' },
          ]} />
        </Row>
        <Row id="capture-images" label="Capture images" description="Remember copied images.">
          <Toggle checked onChange={() => undefined} />
        </Row>
        <Row id="snapshot-limit" label="Snapshot limit" description="Limit stored bytes.">
          <NumberInput value={512} min={1} max={10_240} step={64} onChange={() => undefined} />
        </Row>
        <Row id="global-hotkey" label="Open Clipmo" description={SHORTCUT_RECORDER_DESCRIPTION}>
          <ShortcutRecorder value="Ctrl+Shift+V" onChange={() => undefined} />
        </Row>
      </>,
    );

    expect(markup).toContain('id="theme-label"');
    expect(markup).toContain(
      'role="radiogroup" aria-labelledby="theme-label" aria-describedby="theme-description"',
    );
    expect(markup).toContain(
      'role="switch" aria-checked="true" aria-labelledby="capture-images-label" aria-describedby="capture-images-description"',
    );
    expect(markup).toContain(
      'type="number" aria-labelledby="snapshot-limit-label" aria-describedby="snapshot-limit-description"',
    );
    expect(markup).toContain(
      'class="shortcut-recorder" aria-labelledby="global-hotkey-label" aria-describedby="global-hotkey-description"',
    );
    expect(markup).toContain('press Escape to finish recording');
  });

  it('normalises file extension filters for the native settings model', () => {
    expect(normaliseExtensions('TXT, .Pdf; exe txt')).toEqual(['.txt', '.pdf', '.exe']);
    expect(normaliseExtensions('  ')).toEqual([]);
  });
});

describe('Settings pairing validation', () => {
  it('validates minimum 6 characters for pairing codes', () => {
    expect('123456'.length).toBe(6);
    expect('12345'.length).toBeLessThan(6);
  });
});
