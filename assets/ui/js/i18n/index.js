// Exospine: interface strings in English and French.
//
// English is the canonical locale (en.js); fr.js has exactly the same keys,
// which scripts/check-i18n.mjs verifies in CI.

import en from './en.js';
import fr from './fr.js';

const TABLES = { en, fr };
const SUPPORTED = Object.keys(TABLES);

let currentLang = 'en';
let pluralRules = new Intl.PluralRules('en');

/**
 * The language a preference resolves to: 'auto' (or nothing) follows the
 * system language, and anything without a table falls back to English.
 */
export function resolveLanguage(preference) {
  const tag = !preference || preference === 'auto' ? globalThis.navigator?.language || 'en' : preference;
  const base = String(tag).toLowerCase().split(/[-_]/)[0];
  return SUPPORTED.includes(base) ? base : 'en';
}

/**
 * Switch the interface language. `preference` is 'auto', 'en' or 'fr'. The
 * static strings of the page (see translateStatic) follow at once.
 */
export function setLanguage(preference) {
  currentLang = resolveLanguage(preference);
  pluralRules = new Intl.PluralRules(currentLang);
  if (globalThis.document?.documentElement) {
    document.documentElement.lang = currentLang;
    translateStatic(document);
  }
}

const STATIC_ATTRIBUTES = ['title', 'aria-label', 'placeholder'];

/**
 * Fill in the strings written in HTML: a `data-i18n` attribute names the key
 * of an element's text; `data-i18n-title`, `data-i18n-aria-label` and
 * `data-i18n-placeholder` name the key of that attribute.
 */
export function translateStatic(root) {
  for (const el of root.querySelectorAll('[data-i18n]')) el.textContent = t(el.dataset.i18n);
  for (const attr of STATIC_ATTRIBUTES) {
    for (const el of root.querySelectorAll(`[data-i18n-${attr}]`)) {
      el.setAttribute(attr, t(el.getAttribute(`data-i18n-${attr}`)));
    }
  }
}

/** The current interface language: 'en' or 'fr'. */
export function getLanguage() {
  return currentLang;
}

/** The string for `key` in the current language, or undefined. */
function lookup(key) {
  return TABLES[currentLang][key] ?? en[key];
}

/**
 * The string for `key` in the current language, falling back to English and
 * then to the key itself. `{name}` placeholders take their value from `params`.
 * An `error` param that is itself a key (the backend reports some errors as
 * one, such as accounts_read_only) is translated too.
 */
export function t(key, params) {
  const text = lookup(key) ?? key;
  if (!params) return text;
  const values = { ...params };
  if (values.error !== undefined) values.error = lookup(String(values.error)) ?? values.error;
  return text.replace(/\{(\w+)\}/g, (match, name) => (name in values ? String(values[name]) : match));
}

/**
 * The plural form of `key` for `count`: `<key>_one` or `<key>_other`, chosen
 * by the language's plural rules (French uses the singular for 0 and 1).
 * `{count}` is available to the string.
 */
export function tn(key, count, params) {
  const form = pluralRules.select(count) === 'one' ? 'one' : 'other';
  return t(`${key}_${form}`, { count, ...params });
}
