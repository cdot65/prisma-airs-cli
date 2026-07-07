import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { ManagementService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from '../deprecated-flags.js';
import { fail, ui, usageError } from '../renderer/index.js';

export interface ApplyInput {
  profileName: string;
  topicName: string;
  intent: 'allow' | 'block';
}

export interface ApplyOutput {
  topicId: string;
  topicName: string;
  profileName: string;
  intent: string;
}

export async function applyTopicToProfile(
  mgmt: ManagementService,
  input: ApplyInput,
): Promise<ApplyOutput> {
  const allTopics = await mgmt.listTopics();
  const match = allTopics.find((t) => t.topic_name === input.topicName);
  if (!match || !match.topic_id) {
    throw new Error(`Topic "${input.topicName}" not found. Create it first with "topics create".`);
  }

  const existing = await mgmt.getProfileTopics(input.profileName);

  const merged = existing
    .filter((t) => t.topicName !== input.topicName)
    .map((t) => ({ topicId: t.topicId, topicName: t.topicName, action: t.action }));

  merged.push({
    topicId: match.topic_id,
    topicName: match.topic_name,
    action: input.intent,
  });

  const guardrailAction = input.intent === 'block' ? 'allow' : 'block';

  await mgmt.assignTopicsToProfile(input.profileName, merged, guardrailAction);

  return {
    topicId: match.topic_id,
    topicName: match.topic_name,
    profileName: input.profileName,
    intent: input.intent,
  };
}

export function registerApplyCommand(parent: Command): void {
  const cmd = parent
    .command('apply')
    .description('Assign a topic to a security profile (additive)')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--name <name>', 'Topic name to assign')
    .option('--intent <intent>', 'Topic intent: block or allow', 'block')
    .option('--output <format>', 'Output format: pretty or json', 'pretty');
  registerDeprecatedAlias(cmd, {
    oldFlag: '--format <format>',
    oldKey: 'format',
    canonicalFlag: '--output',
    canonicalKey: 'output',
  });
  cmd.action(async (opts) => {
    resolveDeprecatedAliases(cmd, opts);
    if (opts.intent !== 'allow' && opts.intent !== 'block') {
      usageError(`--intent must be "allow" or "block", got: "${opts.intent}"`);
    }
    try {
      const config = await loadConfig();
      const mgmt = new SdkManagementService({
        clientId: config.mgmtClientId,
        clientSecret: config.mgmtClientSecret,
        tsgId: config.mgmtTsgId,
        tokenEndpoint: config.mgmtTokenEndpoint,
      });

      const result = await applyTopicToProfile(mgmt, {
        profileName: opts.profile,
        topicName: opts.name,
        intent: opts.intent as 'allow' | 'block',
      });

      if (opts.output === 'json') {
        console.log(JSON.stringify(result, null, 2));
      } else {
        ui.keyValue([
          ['Applied', result.topicName],
          ['Profile', result.profileName],
          ['Intent', result.intent],
        ]);
      }
    } catch (err) {
      fail(err);
    }
  });
}
