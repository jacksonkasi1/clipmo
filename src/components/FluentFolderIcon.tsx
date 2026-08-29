import type { CSSProperties } from 'react';

interface FluentFolderIconProps {
  size?: number;
  isOpen?: boolean;
  color?: string;
  className?: string;
  style?: CSSProperties;
}

/**
 * Windows Fluent animated folder icon.
 * Features a layered back tab, an interior sheet that lifts on hover,
 * and a 3D perspective front flap that tilts open.
 */
export function FluentFolderIcon({
  size = 28,
  isOpen = false,
  color,
  className = '',
  style,
}: FluentFolderIconProps) {
  const inlineStyle: CSSProperties = {
    ...(color ? { '--folder-tint': color } as CSSProperties : {}),
    ...style,
  };

  return (
    <span
      className={`fluent-folder-root ${isOpen ? 'is-open' : ''} ${className}`.trim()}
      style={inlineStyle}
      aria-hidden="true"
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 28 28"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="fluent-folder-svg"
      >
        <defs>
          <linearGradient id="fluentFolderBackGrad" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="currentColor" stopOpacity="0.85" />
            <stop offset="100%" stopColor="currentColor" stopOpacity="0.6" />
          </linearGradient>
          <linearGradient id="fluentFolderFrontGrad" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="currentColor" stopOpacity="1" />
            <stop offset="100%" stopColor="currentColor" stopOpacity="0.8" />
          </linearGradient>
          <linearGradient id="fluentFolderSheetGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.92" />
            <stop offset="100%" stopColor="#ffffff" stopOpacity="0.75" />
          </linearGradient>
        </defs>

        {/* Back folder body and top tab */}
        <path
          className="fluent-folder-back"
          d="M3 6.5C3 5.12 4.12 4 5.5 4H10.2C11.1 4 11.95 4.45 12.45 5.2L13.8 7.2C14.15 7.7 14.7 8 15.3 8H22.5C23.88 8 25 9.12 25 10.5V20.5C25 21.88 23.88 23 22.5 23H5.5C4.12 23 3 21.88 3 20.5V6.5Z"
          fill="url(#fluentFolderBackGrad)"
        />

        {/* Interior sheet peeking out */}
        <g className="fluent-folder-sheet">
          <rect
            x="6"
            y="7"
            width="16"
            height="11"
            rx="1.5"
            fill="url(#fluentFolderSheetGrad)"
          />
          <line x1="8.5" y1="10" x2="14" y2="10" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeOpacity="0.45" />
          <line x1="8.5" y1="13" x2="18" y2="13" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeOpacity="0.35" />
        </g>

        {/* Front flap with 3D perspective rotation */}
        <path
          className="fluent-folder-front"
          d="M3 11.5C3 10.12 4.12 9 5.5 9H22.5C23.88 9 25 10.12 25 11.5V20.5C25 21.88 23.88 23 22.5 23H5.5C4.12 23 3 21.88 3 20.5V11.5Z"
          fill="url(#fluentFolderFrontGrad)"
        />

        {/* Subtle highlight line along the top lip of the front flap */}
        <path
          className="fluent-folder-lip"
          d="M4.5 9.5H23.5"
          stroke="rgba(255, 255, 255, 0.4)"
          strokeWidth="0.8"
          strokeLinecap="round"
        />
      </svg>
    </span>
  );
}
