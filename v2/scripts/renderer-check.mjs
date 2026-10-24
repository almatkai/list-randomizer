// Extract and execute the real beta parser/reconciliation functions via TS AST.
// No parser, UI data model or reconciliation behavior is reimplemented/stubbed.
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
const require = createRequire(new URL('../target/v1-check/package.json',import.meta.url));
const ts = require('typescript');
function load(file, names) {
  const text = fs.readFileSync(new URL(`../tests/fixtures/beta/${file}`,import.meta.url),'utf8');
  const ast = ts.createSourceFile(file,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const selected = ast.statements.filter(n => ts.isFunctionDeclaration(n) && names.includes(n.name?.text));
  assert.equal(selected.length,names.length);
  const source = selected.map(n=>n.getText(ast)).join('\n') + `\nglobalThis.api = {${names.join(',')}}`;
  const code = ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  const context = vm.createContext({exports:{}});
  vm.runInContext(code,context);
  return context.api;
}
const list=load('list-runtime.tsx',['parseListRows','cleanSubtitle','parseRowIcon','parseAccessories','parseDetailFromChildren']);
const form=load('form-runtime.tsx',['collectOptionData','collectFormItems','sameFormValue','reconcileFormValues']);
const wire=JSON.parse(fs.readFileSync(new URL('../target/wire.json',import.meta.url),'utf8'));
const plain = value=>JSON.parse(JSON.stringify(value));
const rows=plain(list.parseListRows(wire.list.root));
assert.deepEqual(rows.map(r=>r.title),['','same','same','a,b','Қазақ 😀']);
assert.equal(wire.list.root.props.__hasServerSearch,true);
assert(rows.every(r=>r.actionIds.length===4));
assert.equal(list.parseListRows(wire.empty.root).length,0);
assert.equal(wire.empty.actions[0].title,'Add');
const fields = form.collectFormItems(wire.form.root).filter(i=>i.type==='field').map(i=>i.field);
assert.equal(fields.length,1);
assert.equal(fields[0].actionId,wire.form.root.children[0].props.actionId);
assert.deepEqual(plain(form.reconcileFormValues(fields)),{[fields[0].id]:'queued draft'});
assert.deepEqual(plain(form.reconcileFormValues(fields,{[fields[0].id]:'local'},fields)),{[fields[0].id]:'queued draft'});
const text=fs.readFileSync(new URL('../tests/fixtures/beta/list-runtime.tsx',import.meta.url),'utf8');
assert(text.includes('if (hasServerSearch) return rows'));
console.log('Actual beta List/Form parsers and controlled reconciliation: pass');
