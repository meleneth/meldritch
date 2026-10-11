import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'MELDRITCH',
  description: 'An experimental Rust groovebox. Nothing is stable yet.',
  base: '/meldritch/',
  lastUpdated: true,
  themeConfig: {
    siteTitle: 'MELDRITCH',
    nav: [
      { text: 'Start', link: '/guide/getting-started' },
      { text: 'Controllers', link: '/guide/controllers' },
      { text: 'Lua', link: '/CONTROL_PLANE' },
      { text: 'Development', link: '/guide/status' },
    ],
    sidebar: [
      { text: 'Play & explore', items: [
        { text: 'Getting started', link: '/guide/getting-started' },
        { text: 'Controllers', link: '/guide/controllers' },
        { text: 'Instrument editor', link: '/INSTRUMENT_EDITOR' },
        { text: 'APC piano roll', link: '/PIANO_ROLL' },
        { text: 'APC Mini MK2 playground', link: '/examples/apc-playground' },
        { text: 'Lua APC color demo', link: '/examples/apc-colors' },
        { text: 'Lua APC performance widgets', link: '/examples/lua-apc-performance' },
        { text: 'MIDI fader soft takeover', link: '/examples/midi-soft-takeover' },
        { text: 'MIDI connection recovery', link: '/examples/midi-reconnect' },
        { text: 'Audio-aware Lua widgets', link: '/examples/audio-aware-widgets' },
        { text: 'Example capabilities', link: '/examples/capabilities' },
        { text: 'Lua acceptance example', link: '/examples/lua-control-plane' },
      ] },
      { text: 'Design & contracts', items: [
        { text: 'Architecture', link: '/ARCHITECTURE' },
        { text: 'Project invariants', link: '/INVARIANTS' },
        { text: 'Project formats', link: '/ML_FORMATS' },
        { text: 'Lua API & control plane', link: '/CONTROL_PLANE' },
        { text: 'Modular principles', link: '/EURORACK_PRINCIPLES' },
        { text: 'Numerical policy', link: '/NUMERICS_AND_64BIT' },
      ] },
      { text: 'Development', items: [
        { text: 'Nothing is stable yet', link: '/guide/status' },
        { text: 'Progress & remaining tasks', link: '/TODO' },
        { text: 'Interactive authoring investigation', link: '/AUTHORING' },
        { text: 'Testing strategy', link: '/TESTING_STRATEGY' },
        { text: 'Roadmap', link: '/ROADMAP' },
        { text: 'Contributing & documentation', link: '/guide/contributing' },
      ] },
    ],
    search: { provider: 'local' },
    socialLinks: [{ icon: 'github', link: 'https://github.com/meleneth/meldritch' }],
    editLink: {
      pattern: 'https://github.com/meleneth/meldritch/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
    footer: {
      message: 'Experimental software. APIs, formats, controls and behavior may change.',
    },
  },
})
