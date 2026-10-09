import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import {
  ProfilePolicyError,
  type ProfileSelector,
  validateProfileSelector,
} from '../../airs/profile-policy.js';
import type { ManagementService } from '../../airs/types.js';
import { managementClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { confirmOrAbort } from '../confirm.js';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from '../deprecated-flags.js';
import { fail, ui, usageError } from '../renderer/index.js';

export interface RevertOutput {
  profileName: string;
  deleted: string[];
}

export async function revertTopic(
  mgmt: ManagementService,
  profileName: string,
  topicName: string,
  selector?: ProfileSelector,
): Promise<RevertOutput> {
  const topics = await mgmt.listTopics();
  const match = topics.find((t) => t.topic_name === topicName);
  if (!match || !match.topic_id) {
    throw new Error(`Topic "${topicName}" not found`);
  }

  const profileTopics = await mgmt.getProfileTopics(profileName, selector);
  if (
    !profileTopics.some(
      (topic) => topic.topicId === match.topic_id && topic.topicName === topicName,
    )
  )
    throw new ProfilePolicyError(
      `Topic "${topicName}" is not assigned to the selected protection section`,
    );
  if (selector) {
    const allReferences = await mgmt.getProfileTopics(profileName, undefined, {
      includeInactive: true,
    });
    if (
      allReferences.some(
        (ref) =>
          ref.topicName === topicName &&
          (ref.direction !== selector.direction ||
            (selector.aiProfileIndex !== undefined &&
              (ref.aiProfileIndex ?? 0) !== selector.aiProfileIndex)),
      )
    )
      throw new ProfilePolicyError(
        'Topic is referenced in another direction or AI entry; detach those references before reverting and deleting',
      );
  }
  const remaining = profileTopics
    .filter((t) => t.topicName !== topicName)
    .map((t) => ({ topicId: t.topicId, topicName: t.topicName, action: t.action }));

  const hasBlockTopics = remaining.some((t) => t.action === 'block');
  const guardrailAction = hasBlockTopics ? 'allow' : 'block';
  if (selector) {
    await mgmt.assignTopicsToProfile(profileName, remaining, guardrailAction, selector);
  } else {
    await mgmt.assignTopicsToProfile(profileName, remaining, guardrailAction);
  }
  await mgmt.deleteTopic(match.topic_id);

  return { profileName, deleted: [match.topic_id] };
}

export function registerRevertCommand(parent: Command): void {
  const cmd = parent
    .command('revert')
    .description('Remove a custom topic from a profile and delete it')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--name <name>', 'Topic name to remove')
    .option('--force', 'Skip confirmation prompt')
    .option(
      '--direction <direction>',
      'Protection direction: prompt, response, tool-call, tool-response',
    )
    .option('--ai-profile-index <n>', 'AI profile entry index (zero-based)', Number)
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
      try {
        validateProfileSelector(opts);
      } catch (err) {
        usageError(err instanceof Error ? err.message : String(err));
      }
      await confirmOrAbort(
        `Remove topic "${opts.name}" from profile "${opts.profile}" and delete it?`,
        Boolean(opts.force),
        { action: `revert topic "${opts.name}"` },
      );
      const config = await loadConfig();
      const mgmt = new SdkManagementService(managementClientOptions(config));

      const result = await revertTopic(
        mgmt,
        opts.profile,
        opts.name,
        opts.direction !== undefined || opts.aiProfileIndex !== undefined
          ? { direction: opts.direction, aiProfileIndex: opts.aiProfileIndex }
          : undefined,
      );

      if (opts.output === 'json') {
        console.log(JSON.stringify(result, null, 2));
      } else {
        ui.keyValue([
          ['Reverted', opts.name],
          ['Profile', result.profileName],
          ['Deleted', result.deleted.join(', ')],
        ]);
      }
    } catch (err) {
      fail(err);
    }
  });
}
