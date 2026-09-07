import assert from 'node:assert/strict';
import test from 'node:test';
import { nativeDiagnostics } from './native-diagnostics.mjs';

const warning =
  '(node:2440) ExperimentalWarning: Importing JSON modules is an experimental feature and might change at any time\n(Use `node --trace-warnings ...` to show where the warning was created)\n';

test('empty native stderr needs no exception', () => {
  assert.deepEqual(nativeDiagnostics('', 'v24.0.0'), []);
});
test('the observed Node 20 warning is reported, not suppressed', () => {
  assert.equal(nativeDiagnostics(warning, 'v20.17.0').length, 1);
});
test('other runtimes or diagnostics still fail the native gate', () => {
  assert.throws(() => nativeDiagnostics(warning, 'v24.0.0'));
  assert.throws(() =>
    nativeDiagnostics('Fontconfig error: No writable cache directories\n', 'v20.17.0'),
  );
  assert.throws(() => nativeDiagnostics('ExperimentalWarning: something else\n', 'v20.17.0'));
});
test('a known warning cannot hide an appended or prepended error', () => {
  assert.throws(() => nativeDiagnostics(`${warning}failure\n`, 'v20.17.0'));
  assert.throws(() => nativeDiagnostics(`failure\n${warning}`, 'v20.17.0'));
});
