import { dump as yamlDump } from 'js-yaml';
import type { OutputFormat } from './common.js';

// biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
function emit(payload: any, fmt: OutputFormat): void {
  switch (fmt) {
    case 'json':
      console.log(JSON.stringify(payload, null, 2));
      return;
    case 'yaml':
      console.log(yamlDump(payload));
      return;
    default:
      console.log(typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2));
  }
}

export const dlpFilteringProfiles = {
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderList(page: any, fmt: OutputFormat) {
    emit(page, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderGet(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderReplaced(item: any, fmt: OutputFormat) {
    if (fmt === 'pretty') console.log(`  replaced ${item.id}`);
    else emit(item, fmt);
  },
};

export const dlpPatterns = {
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderList(page: any, fmt: OutputFormat) {
    emit(page, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderCreated(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderGet(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderReplaced(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderPatched(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  renderArchived(id: string) {
    console.log(`  archived ${id}`);
  },
};

export const dlpProfiles = {
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderList(page: any, fmt: OutputFormat) {
    emit(page, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderCreated(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderGet(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderReplaced(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderPatched(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
};

export const dlpDictionaries = {
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderList(page: any, fmt: OutputFormat) {
    emit(page, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderCreated(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderGet(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderReplaced(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  // biome-ignore lint/suspicious/noExplicitAny: renderer accepts arbitrary SDK payloads
  renderPatched(item: any, fmt: OutputFormat) {
    emit(item, fmt);
  },
  renderReplaced204Fallback(id: string) {
    console.log(`  replaced ${id} (state not echoed by region)`);
  },
  renderDeleted(id: string) {
    console.log(`  deleted ${id}`);
  },
};
