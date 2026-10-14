// Exospine: the Colony appearance: theme, accent, high contrast, text sizes,
// the dyslexia font and reduced motion.
//
// The themes, accents and contrast ends come from the vendored Colony tokens
// (vendor/colony, see scripts/sync-colony-tokens.sh): colony-themes.css
// styles [data-colony-theme], and this module sets that attribute and the
// variables derived at runtime. The preference names and values are those of
// colony_ui::preferences::StandardPreferences.

import colony from '../vendor/colony/themes.js';

export const DEFAULT_THEME = Object.freeze({ family: 'gruvbox', variant: 'dark' });
export const FAMILIES = colony.families;
export const ACCENTS = colony.accents;

/** Typography size (Appearance) and text size (Accessibility) factors; they multiply. */
export const FONT_SIZE_FACTORS = Object.freeze({ small: 0.85, default: 1, large: 1.2 });
export const TEXT_SIZE_FACTORS = Object.freeze({ small: 0.85, default: 1, large: 1.2, xlarge: 1.4 });

/** The fields colony_ui's with_high_contrast changes, by how much. */
const HIGH_CONTRAST = Object.freeze({
  text_primary: [0.15, 0.12],
  text_secondary: [0.12, 0.10],
  text_muted: [0.10, 0.10],
  text_dim: [0.10, 0.08],
  text_dimmer: [0.08, 0.08],
  border_subtle: [0.15, 0.12],
  divider: [0.15, 0.12],
});

const CACHE_KEY = 'exospine_appearance';
const reducedMotionQuery = globalThis.matchMedia?.('(prefers-reduced-motion: reduce)');

let current = null;

const cssVar = (field) => `--colony-${field.replace(/_/g, '-')}`;
const channels = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
const toHex = (rgb) => '#' + rgb.map((c) => Math.round(Math.min(1, Math.max(0, c)) * 255).toString(16).padStart(2, '0')).join('');

/** YIQ luma, as colony_ui decides whether a surface is light. */
function isLight(hex) {
  const [r, g, b] = channels(hex);
  return 0.299 * r + 0.587 * g + 0.114 * b > 0.5;
}

/** WCAG contrast ratio of two colours. */
export function contrastRatio(a, b) {
  const lum = (hex) => {
    const [r, g, b] = channels(hex).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** Ink or paper, whichever reads better on `bg` (colony_ui::contrast_on). */
export function contrastOn(bg) {
  const { ink, paper } = colony.contrastEnds;
  return contrastRatio(ink, bg) >= contrastRatio(paper, bg) ? ink : paper;
}

/** The family and variant to show, falling back to Gruvbox dark when unknown. */
export function resolveTheme(familyKey, variantKey) {
  const family = FAMILIES.find((f) => f.key === familyKey);
  const variant = family?.variants.find((v) => v.key === variantKey);
  if (variant) return { family, variant };
  const fallback = FAMILIES.find((f) => f.key === DEFAULT_THEME.family);
  return { family: fallback, variant: fallback.variants.find((v) => v.key === DEFAULT_THEME.variant) };
}

/** A palette with colony_ui's high-contrast boost applied. */
export function withHighContrast(palette) {
  const light = isLight(palette.bg_primary);
  const boosted = { ...palette };
  for (const [field, [darken, brighten]] of Object.entries(HIGH_CONTRAST)) {
    boosted[field] = toHex(channels(palette[field]).map((c) => (light ? c - darken : c + brighten)));
  }
  return boosted;
}

/** The accent override in effect, or null for the theme's own accent. */
export function accentOverride(prefs) {
  if (prefs.auto_accent) return null;
  return ACCENTS.find((a) => a.key === prefs.selected_accent) ?? null;
}

/** The product of the two text-size preferences. */
export function fontScale(prefs) {
  return (FONT_SIZE_FACTORS[prefs.font_size] ?? 1) * (TEXT_SIZE_FACTORS[prefs.text_size_a11y] ?? 1);
}

/** Reduced motion is on when asked for here or in the system settings. */
export function reducedMotion(prefs = current) {
  return Boolean(prefs?.reduce_motion || reducedMotionQuery?.matches);
}

/** Whether animations may run: effects on and no reduced motion (colony_ui::effects_enabled). */
export function effectsEnabled(prefs = current) {
  return (prefs?.animations ?? true) && !reducedMotion(prefs);
}

/** The preferences last applied. */
export function currentAppearance() {
  return current;
}

/**
 * Put `prefs` (the StandardPreferences fields of the settings) into effect on
 * this window, and remember them so the next start, and the message
 * pop-outs, open with them before the settings load.
 */
export function applyAppearance(prefs) {
  current = { ...prefs };
  const root = document.documentElement;
  const { variant } = resolveTheme(prefs.selected_theme, prefs.selected_variant);
  const palette = prefs.high_contrast ? withHighContrast(variant.palette) : variant.palette;

  root.dataset.colonyTheme = variant.slug;
  root.style.colorScheme = variant.mode;
  for (const field of Object.keys(HIGH_CONTRAST)) {
    if (prefs.high_contrast) root.style.setProperty(cssVar(field), palette[field]);
    else root.style.removeProperty(cssVar(field));
  }

  const accent = accentOverride(prefs)?.color ?? palette.accent_blue;
  if (accentOverride(prefs)) root.style.setProperty('--colony-accent-blue', accent);
  else root.style.removeProperty('--colony-accent-blue');
  root.style.setProperty('--colony-ink', colony.contrastEnds.ink);
  root.style.setProperty('--colony-paper', colony.contrastEnds.paper);
  root.style.setProperty('--on-accent', contrastOn(accent));
  root.style.setProperty('--on-error', contrastOn(palette.error));
  root.style.setProperty('--on-success', contrastOn(palette.success));
  root.style.setProperty('--on-warning', contrastOn(palette.warning));
  // The shared accents, for colours that only need to tell things apart
  // (accounts, labels, categories, events), each with its readable label colour.
  for (const a of ACCENTS) {
    root.style.setProperty(`--accent-${a.key}`, a.color);
    root.style.setProperty(`--on-accent-${a.key}`, contrastOn(a.color));
  }

  root.style.setProperty('--font-scale', String(fontScale(prefs)));
  root.dataset.dyslexia = prefs.dyslexia_font ? 'on' : 'off';
  root.dataset.reducedMotion = reducedMotion(prefs) ? 'on' : 'off';
  root.dataset.effects = effectsEnabled(prefs) ? 'on' : 'off';

  try { localStorage.setItem(CACHE_KEY, JSON.stringify(current)); } catch {}
}

/** Apply the preferences remembered from the last run, or the defaults. */
export function restoreAppearance() {
  let cached = null;
  try { cached = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null'); } catch {}
  applyAppearance(cached || {
    selected_theme: DEFAULT_THEME.family,
    selected_variant: DEFAULT_THEME.variant,
    font_size: 'default',
    text_size_a11y: 'default',
    animations: true,
  });
}

// Follow the system's reduced-motion setting while the app runs.
reducedMotionQuery?.addEventListener?.('change', () => {
  if (current) applyAppearance(current);
});

/** The key of the n-th shared accent, for colouring the n-th account or label. */
export function accentKeyAt(index) {
  return ACCENTS[((index % ACCENTS.length) + ACCENTS.length) % ACCENTS.length].key;
}

/** The accent a name always gets, for avatars. */
export function accentFor(text) {
  let hash = 0;
  for (const c of String(text || '')) hash = (hash * 31 + c.charCodeAt(0)) | 0;
  return accentKeyAt(Math.abs(hash));
}

/**
 * A colour as stored with a label or an event: an accent key, or a hex value
 * written by an older version. Anything else gives `fallback`.
 */
export function storedColor(value, fallback = 'var(--accent)') {
  if (ACCENTS.some((a) => a.key === value)) return `var(--accent-${value})`;
  return /^#[0-9a-f]{3,8}$/i.test(String(value || '')) ? value : fallback;
}

/** `storedColor` as a background, with a text colour that reads on it. */
export function storedColorStyle(value) {
  if (ACCENTS.some((a) => a.key === value)) return `background:var(--accent-${value});color:var(--on-accent-${value})`;
  const background = storedColor(value);
  const text = background.startsWith('#') ? contrastOn(background) : 'var(--on-accent)';
  return `background:${background};color:${text}`;
}
