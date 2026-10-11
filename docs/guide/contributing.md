# Contributing and documentation

Read `AGENTS.md` and the [invariants](../INVARIANTS) before changing engine code.
Use the repository's example-first workflow: add a readable example and
`ACCEPTANCE.md`, negative fixtures, implementation and executable tests together.
Update [capability status](../examples/capabilities) and [remaining tasks](../TODO)
in the same slice. Preserve historical fixtures.

## Work on this site

Use Node.js **24 or newer** from the repository root:

```sh
npm ci
npm run docs:dev
npm run docs:build
npm run docs:preview
```

The site uses VitePress 1.6.4. Vite is pinned to 6.4.4 through an npm override
because the default Vite 5 dependency has known development-server advisories.
The lockfile records reproducible public-registry dependencies. Generated output
and `node_modules` are ignored.

Architecture, formats, Lua API, invariants, capabilities and the active queue are
included from repository Markdown. Edit those original files rather than their
site wrappers. The APC and Lua example pages also include their acceptance
documents directly. Broken internal links fail the production build.

## GitHub Pages

`.github/workflows/docs.yml` builds pull requests and deploys pushes to `main`.
The deployment uses the GitHub Pages artifact workflow and the repository base
path `/meldritch/`. No personal access token is required.

Set **Settings → Pages → Build and deployment → Source → GitHub Actions** once
for the repository. The workflow can also be run manually on `main`. The deploy
job needs `pages: write` and `id-token: write`; pull-request builds have only
read access and cannot deploy. Deployment runs through the `github-pages`
environment and honors its configured protection rules.

Expected URL after a successful deployment:
[meleneth.github.io/meldritch/](https://meleneth.github.io/meldritch/).
If the repository is renamed or a custom domain is introduced, update `base` in
the VitePress config. A local build does not prove GitHub Pages is enabled.
