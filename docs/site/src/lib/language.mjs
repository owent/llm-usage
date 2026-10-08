export const preferenceKey = 'llm-usage.docs.language';

export function preferredLanguage(languages, saved) {
  if (saved === 'en' || saved === 'zh-CN') return saved;
  for (const value of languages ?? []) {
    const language = String(value).toLowerCase().split('-')[0];
    if (language === 'zh') return 'zh-CN';
    if (language === 'en') return 'en';
  }
  return 'en';
}

export function localizedPath(pathname, language) {
  const path = pathname.replace(/^\/zh-cn(?=\/|$)/, '') || '/';
  return language === 'zh-CN' ? `/zh-cn${path.startsWith('/') ? path : `/${path}`}` : path;
}
