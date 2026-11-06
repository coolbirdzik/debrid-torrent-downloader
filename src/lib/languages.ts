/**
 * Single registry of UI languages. To add one: add an entry here, add `locales/<code>.json`,
 * import it in i18n.svelte.ts, and allow the code in `crates/core/src/models.rs` (SUPPORTED_LANGUAGES).
 */
export const LANGUAGES = [
  { code: 'vi', label: 'Tiếng Việt', tag: 'vi-VN' },
  { code: 'en', label: 'English', tag: 'en-US' },
  { code: 'zh', label: '简体中文', tag: 'zh-CN' },
  { code: 'tr', label: 'Türkçe', tag: 'tr-TR' },
  { code: 'es', label: 'Español', tag: 'es-ES' },
  { code: 'de', label: 'Deutsch', tag: 'de-DE' },
  { code: 'fr', label: 'Français', tag: 'fr-FR' },
  { code: 'ko', label: '한국어', tag: 'ko-KR' },
  { code: 'ja', label: '日本語', tag: 'ja-JP' },
  { code: 'pt', label: 'Português (Brasil)', tag: 'pt-BR' },
  { code: 'ru', label: 'Русский', tag: 'ru-RU' },
] as const;

export type Language = (typeof LANGUAGES)[number]['code'];

export const isLanguage = (value: unknown): value is Language => LANGUAGES.some((language) => language.code === value);

/** BCP-47 tag for `Intl` / `<html lang>`; falls back to English. */
export const languageTag = (code: string): string => LANGUAGES.find((language) => language.code === code)?.tag ?? 'en-US';
