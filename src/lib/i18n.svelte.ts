import en from './locales/en.json';
import zh from './locales/zh.json';
import tr from './locales/tr.json';
import es from './locales/es.json';
import de from './locales/de.json';
import fr from './locales/fr.json';
import ko from './locales/ko.json';
import ja from './locales/ja.json';
import pt from './locales/pt.json';
import ru from './locales/ru.json';
import { isLanguage, languageTag, type Language } from './languages';
import type { Msg } from './types';

type Catalog = Record<string, string>;
const english = en as Catalog;
/** Vietnamese is the source language and the key, so it has no catalog. */
const catalogs: Partial<Record<Language, Catalog>> = { en: english, zh, tr, es, de, fr, ko, ja, pt, ru };

let language = $state<Language>('en');

export const getLanguage = () => language;
export const setLanguage = (value: string) => {
  language = isLanguage(value) ? value : 'en';
  if (typeof document !== 'undefined') document.documentElement.lang = languageTag(language);
};

/** Vietnamese is the source language and the key; other languages come from their catalog, falling back to English. Reads `language`, so it is reactive. */
export function t(key: string, ...args: (string | number)[]): string {
  const template = language === 'vi' ? key : (catalogs[language]?.[key] ?? english[key] ?? key);
  return template.replace(/\{(\d+)(?::[^}]*)?\}/g, (_, index) => String(args[Number(index)] ?? ''));
}

export const tm = (message: Msg | null | undefined) => (message ? t(message.key, ...message.args) : '');
