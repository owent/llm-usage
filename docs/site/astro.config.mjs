import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://llm-usage.atframe.work',
  trailingSlash: 'always',
  outDir: '../../build/documentation-site/dist',
  cacheDir: '../../build/documentation-site/astro-cache',
  // Bundle each importer's YAML dependency: Starlight uses v4, markdownlint uses v5.
  vite: { environments: { prerender: { resolve: { noExternal: ['js-yaml'] } } } },
  integrations: [starlight({
    title: 'LLM Usage',
    description: 'Understand the AI usage recorded on your computer.',
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
    sidebar: [
      { label: 'Start here', translations: { 'zh-CN': '开始使用' }, items: [{ autogenerate: { directory: 'start' } }] },
      { label: 'User guide', translations: { 'zh-CN': '用户指南' }, items: [{ autogenerate: { directory: 'guide' } }] },
      { label: 'Developer guide', translations: { 'zh-CN': '开发指南' }, items: [{ autogenerate: { directory: 'development' } }] },
      { label: 'Design specifications', translations: { 'zh-CN': '设计说明' }, items: [{ autogenerate: { directory: 'reference/design' } }], collapsed: true },
      { label: 'Development records', translations: { 'zh-CN': '开发验证记录' }, items: [{ autogenerate: { directory: 'reference/evidence' } }], collapsed: true },
      { label: 'Project reference', translations: { 'zh-CN': '项目说明' }, items: [{ autogenerate: { directory: 'reference/repository' } }], collapsed: true },
      { label: 'Test data', translations: { 'zh-CN': '测试数据说明' }, items: [{ autogenerate: { directory: 'reference/fixtures' } }], collapsed: true },
    ],
    editLink: { baseUrl: 'https://github.com/owent/llm-usage/edit/main/' },
    lastUpdated: false,
    tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
  })],
});
