import assert from 'node:assert/strict';

/** Keep stdout strict; disclose the older Node 20 JSON-module warning on stderr. */
export function nativeDiagnostics(stderr, nodeVersion) {
  if (stderr === '') return [];
  const jsonModuleWarning =
    /^\(node:\d+\) ExperimentalWarning: Importing JSON modules is an experimental feature and might change at any time\n\(Use [^\n]*--trace-warnings [^\n]*to show where the warning was created\)\n$/;
  assert.ok(
    nodeVersion.startsWith('v20.') && jsonModuleWarning.test(stderr),
    `Unexpected native CLI diagnostic: ${stderr}`,
  );
  return [
    'Node 20 emitted its dependency JSON-module ExperimentalWarning on stderr; JSON stdout remained valid.',
  ];
}
