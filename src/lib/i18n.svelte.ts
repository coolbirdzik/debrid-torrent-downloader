import catalog from './en.json';
import type { Msg } from './types';

const english = catalog as Record<string, string>;
let language = $state<'en' | 'vi'>('en');

export const getLanguage = () => language;
export const setLanguage = (value: 'en' | 'vi') => {
  language = value === 'vi' ? 'vi' : 'en';
  if (typeof document !== 'undefined') document.documentElement.lang = language;
};

/** Vietnamese is the source language and the key; English comes from the catalog. Reads `language`, so it is reactive. */
export function t(key: string, ...args: (string | number)[]): string {
  const template = language === 'en' ? (english[key] ?? key) : key;
  return template.replace(/\{(\d+)(?::[^}]*)?\}/g, (_, index) => String(args[Number(index)] ?? ''));
}

export const tm = (message: Msg | null | undefined) => (message ? t(message.key, ...message.args) : '');
