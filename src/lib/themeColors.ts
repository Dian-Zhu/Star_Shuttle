export const themeColors = {
  blue: {
    50: '#eff6ff',
    100: '#dbeafe',
    200: '#bfdbfe',
    300: '#93c5fd',
    400: '#60a5fa',
    500: '#3b82f6',
    600: '#2563eb',
    700: '#1d4ed8',
    800: '#1e40af',
    900: '#1e3a8a',
    950: '#172554',
  },
  purple: {
    50: '#faf5ff',
    100: '#f3e8ff',
    200: '#e9d5ff',
    300: '#d8b4fe',
    400: '#c084fc',
    500: '#a855f7',
    600: '#9333ea',
    700: '#7e22ce',
    800: '#6b21a8',
    900: '#581c87',
    950: '#3b0764',
  },
  green: {
    50: '#f0fdf4',
    100: '#dcfce7',
    200: '#bbf7d0',
    300: '#86efac',
    400: '#4ade80',
    500: '#22c55e',
    600: '#16a34a',
    700: '#15803d',
    800: '#166534',
    900: '#14532d',
    950: '#052e16',
  },
  orange: {
    50: '#fff7ed',
    100: '#ffedd5',
    200: '#fed7aa',
    300: '#fdba74',
    400: '#fb923c',
    500: '#f97316',
    600: '#ea580c',
    700: '#c2410c',
    800: '#9a3412',
    900: '#7c2d12',
    950: '#431407',
  },
  red: {
    50: '#fef2f2',
    100: '#fee2e2',
    200: '#fecaca',
    300: '#fca5a5',
    400: '#f87171',
    500: '#ef4444',
    600: '#dc2626',
    700: '#b91c1c',
    800: '#991b1b',
    900: '#7f1d1d',
    950: '#450a0a',
  },
  cyan: {
    50: '#ecfeff',
    100: '#cffafe',
    200: '#a5f3fc',
    300: '#67e8f9',
    400: '#22d3ee',
    500: '#06b6d4',
    600: '#0891b2',
    700: '#0e7490',
    800: '#155e75',
    900: '#164e63',
    950: '#083344',
  },
  pink: {
    50: '#fdf2f8',
    100: '#fce7f3',
    200: '#fbcfe8',
    300: '#f9a8d4',
    400: '#f472b6',
    500: '#ec4899',
    600: '#db2777',
    700: '#be185d',
    800: '#9d174d',
    900: '#831843',
    950: '#500724',
  }
} as const;

export type ThemeColorKey = keyof typeof themeColors;

/** Shade keys of a palette, matching the `--color-primary-{shade}` CSS variables. */
export type ThemeShade = keyof typeof themeColors.blue;

/**
 * How far each shade is mixed away from the base (shade 500):
 * positive = mix toward white (lighter), negative = mix toward black (darker).
 */
const SHADE_MIX: Record<ThemeShade, number> = {
  50: 0.95,
  100: 0.9,
  200: 0.75,
  300: 0.6,
  400: 0.3,
  500: 0,
  600: -0.1,
  700: -0.25,
  800: -0.4,
  900: -0.55,
  950: -0.7,
};

function clampChannel(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function parseHex(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const int = parseInt(m[1], 16);
  return [(int >> 16) & 255, (int >> 8) & 255, int & 255];
}

function toHex(r: number, g: number, b: number): string {
  const h = (n: number) => clampChannel(n).toString(16).padStart(2, '0');
  return `#${h(r)}${h(g)}${h(b)}`;
}

/**
 * Build a full 50–950 shade palette from a single base hex color, so a
 * user-picked accent color can drive the same `--color-primary-*` variables as
 * the named presets. Lighter shades mix toward white, darker toward black.
 * Returns the `blue` palette as a fallback for an unparseable hex.
 */
export function generateAccentShades(hex: string): Record<ThemeShade, string> {
  const rgb = parseHex(hex);
  if (!rgb) return { ...themeColors.blue };
  const [r, g, b] = rgb;

  const shades = {} as Record<ThemeShade, string>;
  for (const [shade, ratio] of Object.entries(SHADE_MIX) as unknown as [ThemeShade, number][]) {
    const target = ratio >= 0 ? 255 : 0;
    const t = Math.abs(ratio);
    shades[shade] = toHex(
      r + (target - r) * t,
      g + (target - g) * t,
      b + (target - b) * t
    );
  }
  return shades;
}

/** True when the stored accent value is a custom hex color rather than a preset key. */
export function isCustomAccent(color: string | undefined): boolean {
  return typeof color === 'string' && parseHex(color) !== null;
}
