import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Meldritch',
  description: 'A programmable environment for creating musical machines.',
  base: '/meldritch/',
  lastUpdated: true,
  rewrites: {
    'README.md': 'index.md',
  },
  srcExclude: [
    'node_modules/**',
    'target/**',
    '.agents/**',
    '.codex/**',
    '.git/**',
  ],
  themeConfig: {
    siteTitle: 'Meldritch',
    nav: [
      { text: 'Vision', link: '/' },
      { text: 'Philosophy', link: '/PHILOSOPHY' },
      { text: 'Architecture', link: '/ARCHITECTURE' },
      { text: 'Acceptance', link: '/examples/ACCEPTANCE' },
    ],
    sidebar: [
      {
        text: 'Design constitution',
        items: [
          { text: 'What Meldritch is', link: '/' },
          { text: 'Philosophy', link: '/PHILOSOPHY' },
          { text: 'Architecture', link: '/ARCHITECTURE' },
          { text: 'Authored-instrument acceptance', link: '/examples/ACCEPTANCE' },
        ],
      },
    ],
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: 'https://github.com/meleneth/meldritch' }],
    editLink: {
      pattern: 'https://github.com/meleneth/meldritch/edit/meldritch2/:path',
      text: 'Edit this page on GitHub',
    },
    footer: {
      message: 'The instrument itself is a programmable act of composition.',
    },
  },
})
