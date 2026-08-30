// ** import types
import type { CSSProperties } from 'react';
import type { ClipItem, CollectionSummary } from '../lib/types';

// ** import lib
import {
  Check,
  Folder,
} from 'lucide-react';

interface PopoutFolderCardProps {
  collection: CollectionSummary;
  items?: ClipItem[];
  color?: string;
  toneIndex?: number;
  isSelected?: boolean;
  onClick: () => void;
  onOpenInHistory: () => void;
}

export type BadgeType =
  | 'figma'
  | 'instagram'
  | 'github'
  | 'vscode'
  | 'greendoc'
  | 'chrome'
  | 'youtube'
  | 'linkedin'
  | 'finance'
  | 'ai'
  | 'fitness'
  | 'recipe'
  | 'terminal'
  | 'pdf'
  | 'image'
  | 'generic';

export interface DocPreview {
  id: string;
  label: string;
  sub: string;
  accent: string;
  badgeType: BadgeType;
  showDocLines?: boolean;
}

/**
 * Maps clip items or collection keywords to high-impact app icons and crossed documents.
 */
export function resolveDocPreviews(collection: CollectionSummary, items: ClipItem[] = []): DocPreview[] {
  const matching = items.filter((item) =>
    item.tags.some((t) => t.trim().toLowerCase() === collection.name.trim().toLowerCase()),
  );

  if (matching.length > 0) {
    const docs: DocPreview[] = [];
    for (const item of matching.slice(0, 2)) {
      const src = (item.source?.name || item.source?.exePath || '').toLowerCase();
      const preview = item.preview || '';

      if (src.includes('figma')) {
        docs.push({ id: `item-${item.id}`, label: 'FIG', sub: 'Figma', accent: '#a259ff', badgeType: 'figma' });
      } else if (src.includes('instagram')) {
        docs.push({ id: `item-${item.id}`, label: 'INSTA', sub: 'Instagram', accent: '#e1306c', badgeType: 'instagram' });
      } else if (src.includes('github')) {
        docs.push({ id: `item-${item.id}`, label: 'GIT', sub: 'GitHub', accent: '#ffffff', badgeType: 'github' });
      } else if (src.includes('code') || src.includes('studio') || preview.includes('const ') || preview.includes('function') || preview.includes('import ')) {
        docs.push({ id: `item-${item.id}`, label: 'CODE', sub: 'VS Code', accent: '#007acc', badgeType: 'vscode' });
      } else if (src.includes('chrome') || src.includes('edge') || src.includes('firefox') || item.kind === 'link') {
        docs.push({ id: `item-${item.id}`, label: 'WEB', sub: 'Browser', accent: '#4285f4', badgeType: 'chrome' });
      } else if (item.kind === 'image') {
        docs.push({ id: `item-${item.id}`, label: 'IMG', sub: 'Image', accent: '#06b6d4', badgeType: 'image' });
      } else if (item.kind === 'files' || preview.toLowerCase().endsWith('.pdf')) {
        docs.push({ id: `item-${item.id}`, label: 'PDF', sub: 'Document', accent: '#ea4335', badgeType: 'pdf', showDocLines: true });
      } else if (src.includes('terminal') || src.includes('powershell') || src.includes('cmd')) {
        docs.push({ id: `item-${item.id}`, label: 'CLI', sub: 'Terminal', accent: '#4af626', badgeType: 'terminal' });
      } else {
        docs.push({ id: `item-${item.id}`, label: 'DOC', sub: item.source?.name || 'Document', accent: '#39b9e8', badgeType: 'generic' });
      }
    }

    if (docs.length === 1) {
      docs.push({ id: 'f-2', label: 'PDF', sub: 'Document', accent: '#30d158', badgeType: 'greendoc', showDocLines: true });
    }
    return docs;
  }

  // Keyword-based smart mapping for empty/preset collections
  const name = collection.name.toLowerCase();

  // 1. Instagram / Social / Media
  if (name.includes('instagram') || name.includes('insta') || name.includes('social')) {
    return [
      { id: 'p-1', label: 'INSTA', sub: 'Instagram', accent: '#e1306c', badgeType: 'instagram' },
      { id: 'p-2', label: 'IMG', sub: 'Photos', accent: '#06b6d4', badgeType: 'image' },
    ];
  }

  // 2. Graphics / Design / Figma / UI / UX / Roast (Exact Match to User Reference Image!)
  if (name.includes('graphic') || name.includes('design') || name.includes('ui') || name.includes('ux') || name.includes('figma') || name.includes('logo') || name.includes('roast') || name.includes('art') || name.includes('sketch')) {
    return [
      { id: 'p-1', label: 'FIG', sub: 'Figma', accent: '#a259ff', badgeType: 'figma' },
      { id: 'p-2', label: 'PDF', sub: 'Specs', accent: '#30d158', badgeType: 'greendoc', showDocLines: true },
    ];
  }

  // 3. GitHub / Git
  if (name.includes('github') || name.includes('repo') || name.includes('commit') || name.includes('pr')) {
    return [
      { id: 'p-1', label: 'GIT', sub: 'GitHub', accent: '#ffffff', badgeType: 'github' },
      { id: 'p-2', label: 'CODE', sub: 'VS Code', accent: '#007acc', badgeType: 'vscode' },
    ];
  }

  // 4. Code / Dev / Work / Scripts / Python / React / Rust
  if (name.includes('code') || name.includes('dev') || name.includes('work') || name.includes('script') || name.includes('react') || name.includes('rust') || name.includes('python') || name.includes('js') || name.includes('ts')) {
    return [
      { id: 'p-1', label: 'CODE', sub: 'VS Code', accent: '#007acc', badgeType: 'vscode' },
      { id: 'p-2', label: 'CLI', sub: 'Terminal', accent: '#4af626', badgeType: 'terminal' },
    ];
  }

  // 5. LinkedIn / Career / Resume / Job
  if (name.includes('linkedin') || name.includes('job') || name.includes('career') || name.includes('resume') || name.includes('profile')) {
    return [
      { id: 'p-1', label: 'LINK', sub: 'LinkedIn', accent: '#0a66c2', badgeType: 'linkedin' },
      { id: 'p-2', label: 'PDF', sub: 'Resume', accent: '#ea4335', badgeType: 'pdf', showDocLines: true },
    ];
  }

  // 6. Expenses / Finance / Money / Tax / Invoice / Bill
  if (name.includes('expense') || name.includes('finance') || name.includes('money') || name.includes('tax') || name.includes('invoice') || name.includes('bill') || name.includes('budget') || name.includes('receipt')) {
    return [
      { id: 'p-1', label: 'EXP', sub: 'Expense', accent: '#10b981', badgeType: 'finance' },
      { id: 'p-2', label: 'PDF', sub: 'Invoice', accent: '#ea4335', badgeType: 'pdf', showDocLines: true },
    ];
  }

  // 7. Recipes / Food / Cooking
  if (name.includes('recipe') || name.includes('cook') || name.includes('food') || name.includes('kitchen') || name.includes('meal') || name.includes('dish') || name.includes('diet')) {
    return [
      { id: 'p-1', label: 'COOK', sub: 'Recipe', accent: '#ff6b6b', badgeType: 'recipe' },
      { id: 'p-2', label: 'DOC', sub: 'Guide', accent: '#30d158', badgeType: 'greendoc', showDocLines: true },
    ];
  }

  // 8. Workouts / Gym / Fitness / Health / Sport
  if (name.includes('workout') || name.includes('gym') || name.includes('fitness') || name.includes('health') || name.includes('sport') || name.includes('exercise') || name.includes('run')) {
    return [
      { id: 'p-1', label: 'FIT', sub: 'Workout', accent: '#ff9500', badgeType: 'fitness' },
      { id: 'p-2', label: 'LOG', sub: 'Routine', accent: '#30d158', badgeType: 'greendoc', showDocLines: true },
    ];
  }

  // 9. AI / Prompts / Notes / GPT / Claude / Gemini
  if (name.includes('ai') || name.includes('prompt') || name.includes('gpt') || name.includes('claude') || name.includes('gemini') || name.includes('note')) {
    return [
      { id: 'p-1', label: 'AI', sub: 'Prompts', accent: '#6366f1', badgeType: 'ai' },
      { id: 'p-2', label: 'DOC', sub: 'Notes', accent: '#30d158', badgeType: 'greendoc', showDocLines: true },
    ];
  }

  // 10. YouTube / Video / Media / Movies
  if (name.includes('youtube') || name.includes('video') || name.includes('media') || name.includes('stream') || name.includes('movie') || name.includes('film')) {
    return [
      { id: 'p-1', label: 'YT', sub: 'YouTube', accent: '#ff0000', badgeType: 'youtube' },
      { id: 'p-2', label: 'FIG', sub: 'Design', accent: '#a259ff', badgeType: 'figma' },
    ];
  }

  // 11. Web / Links / URLs / Bookmarks
  if (name.includes('web') || name.includes('link') || name.includes('url') || name.includes('bookmark') || name.includes('site')) {
    return [
      { id: 'p-1', label: 'WEB', sub: 'Browser', accent: '#4285f4', badgeType: 'chrome' },
      { id: 'p-2', label: 'PDF', sub: 'Document', accent: '#ea4335', badgeType: 'pdf', showDocLines: true },
    ];
  }

  // Default: Exact match to user reference (Large Figma + Green PDF Spec Doc)
  return [
    { id: 'd-1', label: 'FIG', sub: 'Design', accent: '#a259ff', badgeType: 'figma' },
    { id: 'd-2', label: 'PDF', sub: 'Document', accent: '#30d158', badgeType: 'greendoc', showDocLines: true },
  ];
}

/** Renders high-fidelity application squircle badges */
function DocAppBadge({ doc }: { doc: DocPreview }) {
  if (doc.badgeType === 'figma') {
    return (
      <div className="app-squircle app-squircle-figma" title="Figma">
        <svg width="30" height="42" viewBox="0 0 38 57" fill="none" aria-hidden>
          <path d="M19 28.5C19 23.2533 23.2533 19 28.5 19C33.7467 19 38 23.2533 38 28.5C38 33.7467 33.7467 38 28.5 38C23.2533 38 19 33.7467 19 28.5Z" fill="#1ABCFE" />
          <path d="M0 47.5C0 42.2533 4.25329 38 9.5 38H19V47.5C19 52.7467 14.7467 57 9.5 57C4.25329 57 0 52.7467 0 47.5Z" fill="#0ACF83" />
          <path d="M19 0V19H28.5C33.7467 19 38 14.7467 38 9.5C38 4.25329 33.7467 0 28.5 0H19Z" fill="#FF7262" />
          <path d="M0 9.5C0 14.7467 4.25329 19 9.5 19H19V0H9.5C4.25329 0 0 4.25329 0 9.5Z" fill="#F24E1E" />
          <path d="M0 28.5C0 33.7467 4.25329 38 9.5 38H19V19H9.5C4.25329 19 0 23.2533 0 28.5Z" fill="#A259FF" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'instagram') {
    return (
      <div className="app-squircle app-squircle-instagram" title="Instagram">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <rect width="20" height="20" x="2" y="2" rx="5" ry="5" />
          <path d="M16 11.37A4 4 0 1 1 12.63 8 4 4 0 0 1 16 11.37z" />
          <line x1="17.5" x2="17.51" y1="6.5" y2="6.5" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'github') {
    return (
      <div className="app-squircle app-squircle-github" title="GitHub">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="#FFFFFF" aria-hidden>
          <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'greendoc') {
    return (
      <div className="app-squircle app-squircle-green" title="Document / Specs">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" aria-hidden>
          <path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14v-4z" fill="#FFFFFF" />
          <rect x="3" y="6" width="12" height="12" rx="3" fill="#FFFFFF" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'vscode') {
    return (
      <div className="app-squircle app-squircle-dark" title="VS Code">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden>
          <path d="M17.5 2.5L7 10.5L3 7.5L1.5 8.5V15.5L3 16.5L7 13.5L17.5 21.5L22.5 19V5L17.5 2.5ZM17.5 17.5L8.5 12L17.5 6.5V17.5Z" fill="#007ACC" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'youtube') {
    return (
      <div className="app-squircle app-squircle-red" title="YouTube">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden>
          <path d="M22.54 6.42a2.78 2.78 0 0 0-1.94-2C18.88 4 12 4 12 4s-6.88 0-8.6.46a2.78 2.78 0 0 0-1.94 2A29 29 0 0 0 1 11.75a29 29 0 0 0 .46 5.33A2.78 2.78 0 0 0 3.4 19c1.72.46 8.6.46 8.6.46s6.88 0 8.6-.46a2.78 2.78 0 0 0 1.94-2 29 29 0 0 0 .46-5.25 29 29 0 0 0-.46-5.33z" fill="#FF0000" />
          <polygon points="9.75 15.02 15.5 11.75 9.75 8.48 9.75 15.02" fill="#FFFFFF" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'linkedin') {
    return (
      <div className="app-squircle app-squircle-blue" title="LinkedIn">
        <span style={{ fontSize: '20px', fontWeight: 900, color: '#FFFFFF', letterSpacing: '-0.03em' }}>in</span>
      </div>
    );
  }

  if (doc.badgeType === 'finance') {
    return (
      <div className="app-squircle app-squircle-emerald" title="Finance">
        <span style={{ fontSize: '23px', fontWeight: 900, color: '#FFFFFF' }}>$</span>
      </div>
    );
  }

  if (doc.badgeType === 'ai') {
    return (
      <div className="app-squircle app-squircle-indigo" title="AI Prompts">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#FFD60A" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" fill="#FFD60A" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'fitness') {
    return (
      <div className="app-squircle app-squircle-amber" title="Fitness / Workout">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="#FFFFFF" aria-hidden>
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'recipe') {
    return (
      <div className="app-squircle app-squircle-coral" title="Recipes / Food">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <path d="M18 8h1a4 4 0 0 1 0 8h-1" />
          <path d="M2 8h16v9a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4V8z" />
          <line x1="6" x2="6" y1="1" y2="4" />
          <line x1="10" x2="10" y1="1" y2="4" />
          <line x1="14" x2="14" y1="1" y2="4" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'terminal') {
    return (
      <div className="app-squircle app-squircle-dark" title="Terminal">
        <span style={{ fontSize: '18px', fontWeight: 900, color: '#4af626', fontFamily: 'monospace' }}>&gt;_</span>
      </div>
    );
  }

  if (doc.badgeType === 'chrome') {
    return (
      <div className="app-squircle app-squircle-white" title="Web Browser">
        <svg width="30" height="30" viewBox="0 0 24 24" fill="none" aria-hidden>
          <circle cx="12" cy="12" r="10" fill="#4285F4" opacity="0.18" />
          <circle cx="12" cy="12" r="8.5" stroke="#4285F4" strokeWidth="2.5" />
          <circle cx="12" cy="12" r="3.5" fill="#4285F4" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'pdf') {
    return (
      <div className="app-squircle app-squircle-red" title="PDF Document">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden>
          <rect x="3" y="3" width="18" height="18" rx="4" fill="#EA4335" />
          <path d="M7 8H17M7 12H17M7 16H13" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" />
        </svg>
      </div>
    );
  }

  if (doc.badgeType === 'image') {
    return (
      <div className="app-squircle app-squircle-teal" title="Image">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <rect width="18" height="18" x="3" y="3" rx="3" ry="3" />
          <circle cx="9" cy="9" r="2" fill="#FFFFFF" />
          <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" />
        </svg>
      </div>
    );
  }

  return (
    <div className="app-squircle app-squircle-dark">
      <span style={{ fontSize: '15px', fontWeight: 800, color: '#FFFFFF' }}>TXT</span>
    </div>
  );
}

export function PopoutFolderCard({
  collection,
  items = [],
  color,
  toneIndex = 0,
  isSelected = false,
  onClick,
  onOpenInHistory,
}: PopoutFolderCardProps) {
  const docs = resolveDocPreviews(collection, items);
  const inlineStyle: CSSProperties = {
    ...(color ? { '--folder-color': color } as CSSProperties : {}),
  };

  return (
    <div
      className={`popout-folder-card collection-tone-${toneIndex} ${isSelected ? 'is-selected' : ''}`}
      style={inlineStyle}
      onClick={onClick}
      onDoubleClick={onOpenInHistory}
      tabIndex={0}
      role="button"
      aria-pressed={isSelected}
      aria-label={`Collection ${collection.name}, ${collection.itemCount} ${collection.itemCount === 1 ? 'item' : 'items'}`}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          onOpenInHistory();
        } else if (e.key === ' ') {
          e.preventDefault();
          onClick();
        }
      }}
    >
      {isSelected && <span className="folder-selection-mark" aria-hidden><Check size={12} /></span>}
      {/* 3D Popout Folder Stage */}
      <div className="folder-stage">
        {/* Back Plate with Top Tab */}
        <div className="folder-back">
          <div className="folder-tab" />
        </div>

        {/* 2 Big Crossed Document Sheets Originating from Center */}
        <div className="folder-docs">
          {/* Back Right Document (Crossed Right with App Badge & Spec Lines) */}
          <div className="pop-doc doc-cross-right">
            <div className="doc-fold" />
            {docs[1]?.showDocLines && (
              <div className="doc-lines-preview">
                <div className="doc-line doc-line-title" />
                <div className="doc-line doc-line-sub" />
                <div className="doc-line doc-line-sub" />
                <div className="doc-line doc-line-short" />
              </div>
            )}
            <div className="doc-icon-badge">
              {docs[1] && <DocAppBadge doc={docs[1]} />}
            </div>
            <span className="doc-tag">{docs[1]?.label}</span>
          </div>

          {/* Front Left Document (Crossed Left with App Badge) */}
          <div className="pop-doc doc-cross-left">
            <div className="doc-fold" />
            <div className="doc-icon-badge">
              {docs[0] && <DocAppBadge doc={docs[0]} />}
            </div>
            <span className="doc-tag">{docs[0]?.label}</span>
          </div>
        </div>

        {/* Front Folder Flap (Lower profile, open mouth, hinges open downwards in 3D) */}
        <div className="folder-front">
          <div className="folder-front-lip" />
          <div className="folder-embossed-wrap">
            <span className="folder-embossed-name">{collection.name}</span>
          </div>
          <div className="folder-front-shine" />
        </div>
      </div>

      {/* Card Info Footer */}
      <div className="folder-card-footer">
        <span className="folder-card-name" title={collection.name}>
          <Folder size={14} className="folder-card-icon" fill="currentColor" aria-hidden />
          <strong>{collection.name}</strong>
        </span>
        <span className="folder-card-count">
          {collection.itemCount} {collection.itemCount === 1 ? 'item' : 'items'}
        </span>
      </div>
    </div>
  );
}
