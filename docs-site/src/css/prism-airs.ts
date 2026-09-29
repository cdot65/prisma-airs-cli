import type {PrismTheme} from 'prism-react-renderer';
const airsTheme: PrismTheme = {
  plain: {color: '#f5f8fa', backgroundColor: '#061b29'},
  styles: [
    {types: ['comment', 'prolog', 'cdata'], style: {color: '#8999a6', fontStyle: 'italic'}},
    {types: ['punctuation'], style: {color: '#b6c4cf'}},
    {types: ['keyword', 'tag', 'selector', 'important', 'atrule'], style: {color: '#00ddf2'}},
    {types: ['string', 'char', 'attr-value', 'regex', 'inserted'], style: {color: '#19bda7'}},
    {types: ['function', 'function-variable', 'method'], style: {color: '#ffd348'}},
    {types: ['number', 'boolean', 'constant', 'symbol'], style: {color: '#ff8a1f'}},
    {types: ['operator', 'entity', 'url', 'builtin', 'namespace'], style: {color: '#79e9f4'}},
    {types: ['class-name', 'maybe-class-name', 'changed'], style: {color: '#ffd348'}},
    {types: ['variable', 'attr-name', 'property'], style: {color: '#67caff'}},
    {types: ['deleted'], style: {color: '#ff827a'}},
  ],
};
export default airsTheme;
