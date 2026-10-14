// Unit checks for the web UI logic that has no DOM in it: the theme
// fallback, the high-contrast port of colony-ui, accent contrast, text
// scaling and the i18n plural and placeholder rules.
//
// Run from the repository root: node scripts/test-ui.mjs

import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';

const ui = (path) => import(pathToFileURL(`assets/ui/js/${path}`));
const appearance = await ui('appearance.js');
const i18n = await ui('i18n/index.js');

// An unknown theme falls back to Gruvbox dark; a known one is kept.
assert.equal(appearance.resolveTheme('nope', 'dark').variant.slug, 'gruvbox-dark');
assert.equal(appearance.resolveTheme('gruvbox', 'nope').variant.slug, 'gruvbox-dark');
assert.equal(appearance.resolveTheme('nord', 'light').variant.slug, 'nord-light');

// High contrast brightens the text of a dark theme and darkens a light one,
// and leaves the backgrounds alone, as colony_ui's with_high_contrast does.
const dark = appearance.resolveTheme('gruvbox', 'dark').variant.palette;
const light = appearance.resolveTheme('gruvbox', 'light').variant.palette;
const lum = (hex) => [1, 3, 5].reduce((sum, i) => sum + parseInt(hex.slice(i, i + 2), 16), 0);
assert.ok(lum(appearance.withHighContrast(dark).text_primary) > lum(dark.text_primary));
assert.ok(lum(appearance.withHighContrast(light).text_primary) < lum(light.text_primary));
assert.equal(appearance.withHighContrast(dark).bg_primary, dark.bg_primary);
assert.match(appearance.withHighContrast(dark).divider, /^#[0-9a-f]{6}$/);

// Labels on an accent pick the more legible end, and reach 4:1 on every accent.
for (const accent of appearance.ACCENTS) {
  const ink = appearance.contrastOn(accent.color);
  assert.ok(appearance.contrastRatio(ink, accent.color) >= 4, accent.key);
}

// The two text sizes multiply, from 0.7225 to 1.68.
assert.equal(appearance.fontScale({ font_size: 'default', text_size_a11y: 'default' }), 1);
assert.ok(Math.abs(appearance.fontScale({ font_size: 'small', text_size_a11y: 'small' }) - 0.7225) < 1e-9);
assert.ok(Math.abs(appearance.fontScale({ font_size: 'large', text_size_a11y: 'xlarge' }) - 1.68) < 1e-9);
assert.equal(appearance.fontScale({}), 1);

// An unset or "auto" accent keeps the theme's own accent.
assert.equal(appearance.accentOverride({ selected_accent: 'violet', auto_accent: true }), null);
assert.equal(appearance.accentOverride({ selected_accent: 'violet' }).key, 'violet');
assert.equal(appearance.accentOverride({}), null);

// Language resolution, plurals (French uses the singular for 0 and 1),
// placeholders, shared Colony labels and error codes.
assert.equal(i18n.resolveLanguage('fr-CA'), 'fr');
assert.equal(i18n.resolveLanguage('de'), 'en');
i18n.setLanguage('fr');
assert.equal(i18n.tn('batch_archived', 0), '0 message archivé.');
assert.equal(i18n.tn('batch_archived', 2), '2 messages archivés.');
assert.equal(i18n.t('settings_title'), 'Préférences');
assert.match(i18n.t('error_archive', { error: 'accounts_read_only' }), /liste de comptes/);
i18n.setLanguage('en');
assert.equal(i18n.tn('batch_archived', 0), '0 emails archived.');
assert.equal(i18n.t('moved_to', { folder: 'Work' }), 'Moved to Work');
assert.equal(i18n.t('no_such_key'), 'no_such_key');

console.log('UI checks passed');
