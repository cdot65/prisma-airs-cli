import { managementClientOptions } from '../../../config/client-options.js';
import { loadConfig } from '../../../config/loader.js';

/** Resolve selected-tenant credentials and endpoint overrides before constructing DLP services. */
export async function loadDlpClientOptions() {
  return managementClientOptions(await loadConfig());
}
