'use strict';
const assert = require('node:assert/strict');
const { WikiText } = require('./index');
const wiki = new WikiText(process.argv[2]);
const seen = [];
const request = { source: 'wikitext', target: 'html', input: '{{Greeting}}' };
const result = wiki.convert(request, (operation, title) => {
  seen.push([operation, title]);
  const nested = wiki.convert({ source: 'wikitext', target: 'html', input: 'Nested' });
  assert.equal(nested.ok, true, JSON.stringify(nested));
  assert.match(nested.content, /Nested/);
  return operation === 'template_text' ? 'Hello from Node' : null;
});
assert.equal(result.ok, true, JSON.stringify(result));
assert.match(result.content, /Hello from Node/);
assert(seen.some(([operation]) => operation === 'template_text'));
assert('metadata' in result && 'diagnostics' in result);
const failed = wiki.convert(request, () => { throw new Error('Node lookup failed'); });
assert.equal(failed.ok, false, JSON.stringify(failed));
assert.match(failed.error, /Node lookup failed/);
console.log('Node callback success and failure verified');

const inputs = { namumark: "== 제목 ==\n'''안녕'''", wikitext: "== 제목 ==\n'''안녕'''", gfm: '## 제목\n\n**안녕**', html: '<h2>제목</h2><p><strong>안녕</strong></p>' };
const markers = { namumark: ['==', "'''"], wikitext: ['==', "'''"], gfm: ['##', '**'], html: ['<h2', '<strong'] };
for (const other of ['wikitext', 'gfm', 'html']) for (const [source, target] of [['namumark', other], [other, 'namumark']]) {
  const converted = wiki.convert({ source, target, input: inputs[source] });
  assert.equal(converted.ok, true, JSON.stringify(converted));
  for (const part of ['제목', '안녕', ...markers[target]]) assert(converted.content.includes(part), JSON.stringify(converted));
}

let namuCalls = 0;
const unexpectedLookup = () => { namuCalls++; return null; };
const category = wiki.convert({source: 'namumark', target: 'gfm', input: '[[분류:Example|Sort]]'}, unexpectedLookup);
assert.equal(category.ok, true);
assert.deepEqual(category.metadata.categories, ['Example']);
assert.deepEqual(category.metadata.category_sort_keys, ['Sort']);
assert(category.diagnostics.some(d => d.kind === 'conversion_loss'));
const strict = wiki.convert({source: 'namumark', target: 'html', input: '||cell||', mode: 'strict'}, unexpectedLookup);
assert.equal(strict.ok, false);
assert(strict.diagnostics.some(d => d.kind === 'conversion_loss'));
assert.equal(wiki.convert({source: 'namumark', target: 'html', input: '{{{literal}}}'}, unexpectedLookup).ok, true);
assert.equal(namuCalls, 0);
