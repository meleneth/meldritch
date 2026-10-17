import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Meldritch 2',
  description: 'The active Rust modular synthesis project, with Meldritch 1 preserved as research.',
  base: '/meldritch/',
  lastUpdated: true,
  themeConfig: {
    siteTitle: 'Meldritch 2',
    nav: [
      { text: 'Start', link: '/meldritch2' },
      { text: 'Architecture', link: '/meldritch2/architecture' },
      { text: 'Examples', link: '/meldritch2/examples' },
      { text: 'Meldritch 1 archive', link: '/archive/meldritch1' },
    ],
    sidebar: [
      { text: 'Meldritch 2', items: [
        { text: 'Getting started', link: '/meldritch2' },
        { text: 'Architecture', link: '/meldritch2/architecture' },
        { text: 'Executable examples', link: '/meldritch2/examples' },
      ] },
      { text: 'Completed research', items: [
        { text: 'Meldritch 1 archive', link: '/archive/meldritch1' },
        { text: 'Historical architecture', link: '/ARCHITECTURE' },
        { text: 'Historical example capabilities', link: '/examples/capabilities' },
      ] },
    ],
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: 'https://github.com/meleneth/meldritch' }],
    editLink: {
      pattern: 'https://github.com/meleneth/meldritch/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
    footer: {
      message: 'Meldritch 2 is active. Meldritch 1 is preserved as completed research.',
    },
  },
})
