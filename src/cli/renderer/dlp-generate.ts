import type { GenerateSummary } from '../../dlp/index.js';
import type { OutputFormat } from './common.js';
import { ui } from './ui.js';

/** Render generation data without mixing decoration into machine-readable stdout. */
export function renderGenerateSummary(summary: GenerateSummary, output: OutputFormat): void {
  if (output === 'json') {
    console.log(JSON.stringify(summary, null, 2));
    return;
  }
  ui.header('DLP Test-File Generation');
  ui.keyValue([
    ['Output', summary.out],
    ['Seed', summary.seed],
    ['Clean', summary.clean],
    ['Dirty', summary.dirty],
    ['Manifest', summary.manifestPath],
  ]);
  ui.table(
    [
      { key: 'format', label: 'Format' },
      { key: 'clean', label: 'Clean' },
      { key: 'dirty', label: 'Dirty' },
    ],
    Object.entries(summary.byFormat).map(([format, counts]) => ({ format, ...counts })),
  );
}
