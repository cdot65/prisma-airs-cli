import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { ManagementService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { validateTopic } from '../../core/constraints.js';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from '../deprecated-flags.js';
import { fail, ui } from '../renderer/index.js';

export interface CreateInput {
  name: string;
  description: string;
  examples: string[];
}

export interface CreateOutput {
  topicId: string;
  topicName: string;
  revision: number;
  created: boolean;
}

export async function createOrUpdateTopic(
  mgmt: ManagementService,
  input: CreateInput,
): Promise<CreateOutput> {
  const topic = { name: input.name, description: input.description, examples: input.examples };
  const errors = validateTopic(topic);
  if (errors.length > 0) {
    throw new Error(errors.map((e) => `${e.field}: ${e.message}`).join('; '));
  }

  const existing = await mgmt.listTopics();
  const match = existing.find((t) => t.topic_name === input.name);

  const request = {
    topic_name: input.name,
    description: input.description,
    examples: input.examples,
  };

  if (match && !match.topic_id) {
    throw new Error(`Existing topic '${input.name}' has no topic_id`);
  }

  const result = match
    ? await mgmt.updateTopic(match.topic_id as string, request)
    : await mgmt.createTopic(request);

  if (!result.topic_id) {
    throw new Error('API response missing topic_id');
  }

  return {
    topicId: result.topic_id,
    topicName: result.topic_name,
    revision: result.revision ?? 0,
    created: !match,
  };
}

export function registerCreateCommand(parent: Command): void {
  const cmd = parent
    .command('create')
    .description('Create or update a custom topic definition')
    .requiredOption('--name <name>', 'Topic name')
    .requiredOption('--description <desc>', 'Topic description')
    .requiredOption('--examples <examples...>', 'Example prompts (2-5 required)')
    .option('--output <format>', 'Output format: pretty or json', 'pretty');
  registerDeprecatedAlias(cmd, {
    oldFlag: '--format <format>',
    oldKey: 'format',
    canonicalFlag: '--output',
    canonicalKey: 'output',
  });
  cmd.action(async (opts) => {
    resolveDeprecatedAliases(cmd, opts);
    try {
      const config = await loadConfig();
      const mgmt = new SdkManagementService({
        clientId: config.mgmtClientId,
        clientSecret: config.mgmtClientSecret,
        tsgId: config.mgmtTsgId,
        tokenEndpoint: config.mgmtTokenEndpoint,
      });

      const result = await createOrUpdateTopic(mgmt, {
        name: opts.name,
        description: opts.description,
        examples: opts.examples,
      });

      // 'terminal' is the legacy spelling of the default human-readable format.
      if (opts.output === 'json') {
        console.log(JSON.stringify(result, null, 2));
      } else {
        ui.success(`Topic ${result.created ? 'created' : 'updated'}: ${result.topicName}`);
        ui.keyValue([
          ['ID', result.topicId],
          ['Revision', result.revision],
        ]);
      }
    } catch (err) {
      fail(err);
    }
  });
}
