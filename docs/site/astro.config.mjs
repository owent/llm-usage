import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { documentationSidebar } from './scripts/sidebar.mjs';

export default defineConfig({
  site: 'https://llm-usage.atframe.work',
  trailingSlash: 'always',
  outDir: '../../build/documentation-site/dist',
  cacheDir: '../../build/documentation-site/astro-cache',
  // Bundle each importer's YAML dependency: Starlight uses v4, markdownlint uses v5.
  vite: { environments: { prerender: { resolve: { noExternal: ['js-yaml'] } } } },
  integrations: [starlight({
    title: { en: 'AI usage dashboard', 'zh-CN': 'AI 用量看板' },
    description: 'Explore AI agent tokens, calls and cache usage.',
    logo: { src: './public/brand.svg', replacesTitle: false },
    favicon: '/favicon.svg',
    defaultLocale: 'root',
    locales: {
      root: { label: 'English', lang: 'en' },
      'zh-cn': { label: '简体中文', lang: 'zh-CN' },
    },
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/owent/llm-usage' }],
    customCss: ['./src/styles/custom.css'],
    components: {
      Head: './src/components/Head.astro',
      LanguageSelect: './src/components/LanguageSelect.astro',
      ThemeProvider: './src/components/ThemeProvider.astro',
      ThemeSelect: './src/components/ThemeSelect.astro',
    },
    sidebar: await documentationSidebar(),
    editLink: { baseUrl: 'https://github.com/owent/llm-usage/edit/main/' },
    lastUpdated: false,
    tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
  })],
});
