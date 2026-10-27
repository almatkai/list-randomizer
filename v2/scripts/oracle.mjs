// Runs the exact upstream shuffle body; only its one TS annotation is removed.
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const original = new URL('../tests/fixtures/original/index.tsx', import.meta.url);
const source = fs.readFileSync(original, 'utf8');
const body = source.slice(source.indexOf('function shuffle('), source.indexOf('export default'))
  .replace('arr: Array<string>', 'arr');
assert(body.includes('Math.floor(Math.random() * (i + 1))'));
const inputs = [[], [''], ['same','same',' ','','a,b','Қазақ','日本 😀'], ['a','b','c','d'], [',', ',,', '\n', '  ']];
const cases = [];
for (const input of inputs) {
  for (const sample of [0, 0.499999999, 0.9999999999999999]) {
    const tape = Array.from({length: Math.max(0, input.length - 1)}, (_, i) => (i % 2 ? sample / 2 : sample));
    let consumed = 0;
    const math = Object.create(Math);
    math.random = () => { assert(consumed < tape.length); return tape[consumed++]; };
    const output = JSON.parse(JSON.stringify(vm.runInNewContext(`${body}; shuffle(input)`, {input, Math:math})));
    assert.equal(consumed, tape.length);
    assert.deepEqual([...output].sort(), [...input].sort());
    const content = output.length ? output.join(',') : null;
    cases.push({input, tape, output, content, copy:output.join(', '), restart:content === null ? [] : content.split(',')});
  }
}
const result = JSON.stringify({source:'raycast/extensions@6734d85992290066fd31a7f5bee3cca3d3545a1a/extensions/list-randomizer/src/index.tsx', cases}, null, 2) + '\n';
const target = new URL('../tests/fixtures/oracle.json', import.meta.url);
if (process.argv.includes('--check')) assert.equal(fs.readFileSync(target, 'utf8'), result);
else fs.writeFileSync(target, result);
console.log(`Original JS oracle: ${cases.length} cases verified`);
