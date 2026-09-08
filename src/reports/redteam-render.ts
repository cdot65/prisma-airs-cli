import {
  renderEnvironmentReportHtml,
  renderEnvironmentReportMarkdown,
} from './environment-render.js';
import type { RedTeamEnvironmentReport } from './redteam.js';

/** Portable Markdown using the same allowlisted evidence as the HTML dashboard. */
export function renderRedTeamReportMarkdown(report: RedTeamEnvironmentReport): string {
  return renderEnvironmentReportMarkdown(report);
}

/** Self-contained HTML with hash-based CSP and no remote resources. */
export function renderRedTeamReportHtml(report: RedTeamEnvironmentReport): string {
  return renderEnvironmentReportHtml(report);
}
