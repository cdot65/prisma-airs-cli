import type * as Preset from '@docusaurus/preset-classic';
import type { Config } from '@docusaurus/types';
// Gruvbox dark (hard contrast) syntax theme — see src/css/prism-gruvbox.js
import gruvboxTheme from './src/css/prism-gruvbox';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: 'Prisma AIRS CLI',
  tagline:
    'CLI and library for Palo Alto Prisma AIRS — guardrail refinement, AI red teaming, model security scanning, profile audits',
  favicon: 'img/logo.svg',

  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  // Production url + base path for GitHub Pages (https://cdot65.github.io/prisma-airs-cli/).
  url: 'https://cdot65.github.io',
  baseUrl: '/prisma-airs-cli/',

  organizationName: 'cdot65',
  projectName: 'prisma-airs-cli',

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  markdown: {
    // `.md` -> CommonMark (safe for raw `<`/`{` in generated typedoc/CLI output),
    // `.mdx` -> full MDX (used only by the tab pages that import Tabs/TabItem).
    format: 'detect',
    mermaid: true,
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

  themes: ['@docusaurus/theme-mermaid'],

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          // Docs are served at the site root to preserve the existing mkdocs URLs.
          routeBasePath: '/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/social-card.png',
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    navbar: {
      title: 'Prisma AIRS CLI',
      logo: {
        alt: 'Prisma AIRS CLI',
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docs',
          position: 'left',
          label: 'Docs',
        },
        {
          type: 'docSidebar',
          sidebarId: 'cli',
          position: 'left',
          label: 'CLI Reference',
        },
        {
          type: 'docSidebar',
          sidebarId: 'developers',
          position: 'left',
          label: 'Developers',
        },
        {
          href: 'https://github.com/cdot65/prisma-airs-cli',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            { label: 'Getting Started', to: '/getting-started/installation' },
            { label: 'CLI Reference', to: '/cli/' },
            { label: 'Library', to: '/developers/library/getting-started' },
          ],
        },
        {
          title: 'More',
          items: [
            { label: 'GitHub', href: 'https://github.com/cdot65/prisma-airs-cli' },
            { label: 'npm', href: 'https://www.npmjs.com/package/@cdot65/prisma-airs-cli' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} cdot65. Built with Docusaurus.`,
    },
    prism: {
      theme: gruvboxTheme,
      darkTheme: gruvboxTheme,
      additionalLanguages: ['bash', 'json', 'yaml', 'python', 'powershell', 'toml', 'diff'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
