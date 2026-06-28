/**
 * Gruvbox dark (hard contrast) syntax theme for Docusaurus code blocks.
 * Built from the design system's syntax tokens (tokens/colors.css).
 *
 * Wire into docusaurus.config.js:
 *   const gruvboxTheme = require('./src/css/prism-gruvbox');
 *   ...
 *   themeConfig: {
 *     prism: { theme: gruvboxTheme, darkTheme: gruvboxTheme,
 *              additionalLanguages: ['bash', 'json', 'diff'] },
 *   }
 * (ESM config: `import gruvboxTheme from './src/css/prism-gruvbox.js';`)
 */
const gruvboxTheme = {
  plain: {
    color: '#ebdbb2',          // fg1
    backgroundColor: '#282828', // bg0
  },
  styles: [
    { types: ['comment', 'prolog', 'cdata'], style: { color: '#928374', fontStyle: 'italic' } },
    { types: ['punctuation'], style: { color: '#bdae93' } },
    { types: ['keyword', 'tag', 'selector', 'important', 'atrule'], style: { color: '#fb4934' } },
    { types: ['string', 'char', 'attr-value', 'regex'], style: { color: '#b8bb26' } },
    { types: ['function', 'function-variable', 'method'], style: { color: '#b8bb26' } },
    { types: ['number', 'boolean', 'constant', 'symbol'], style: { color: '#d3869b' } },
    { types: ['operator', 'entity', 'url'], style: { color: '#fe8019' } },
    { types: ['class-name', 'maybe-class-name'], style: { color: '#fabd2f' } },
    { types: ['builtin', 'namespace'], style: { color: '#8ec07c' } },
    { types: ['variable', 'attr-name', 'property'], style: { color: '#83a598' } },
    { types: ['deleted'], style: { color: '#fb4934' } },
    { types: ['inserted'], style: { color: '#b8bb26' } },
    { types: ['changed'], style: { color: '#fabd2f' } },
  ],
};

module.exports = gruvboxTheme;
