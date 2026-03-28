import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import type { ManagementService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { renderError } from '../renderer/index.js';

export interface RevertOutput {
  profileName: string;
  deleted: string[];
}

export async function revertTopic(
  mgmt: ManagementService,
  profileName: string,
  topicName: string,
): Promise<RevertOutput> {
  const topics = await mgmt.listTopics();
  const match = topics.find((t) => t.topic_name === topicName);
  if (!match || !match.topic_id) {
    throw new Error(`Topic "${topicName}" not found`);
  }

  const profileTopics = await mgmt.getProfileTopics(profileName);
  const remaining = profileTopics
    .filter((t) => t.topicName !== topicName)
    .map((t) => ({ topicId: t.topicId, topicName: t.topicName, action: t.action }));

  await mgmt.assignTopicsToProfile(profileName, remaining);

  await mgmt.forceDeleteTopic(match.topic_id, undefined);

  return { profileName, deleted: [match.topic_id] };
}

export function registerRevertCommand(parent: Command): void {
  parent
    .command('revert')
    .description('Remove a custom topic from a profile and delete it')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--name <name>', 'Topic name to remove')
    .option('--format <format>', 'Output format: json or terminal', 'terminal')
    .action(async (opts) => {
      try {
        const config = await loadConfig();
        const mgmt = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        const result = await revertTopic(mgmt, opts.profile, opts.name);

        if (opts.format === 'json') {
          console.log(JSON.stringify(result, null, 2));
        } else {
          console.log(`\n  Reverted: ${opts.name}`);
          console.log(`  Profile:  ${result.profileName}`);
          console.log(`  Deleted:  ${result.deleted.join(', ')}\n`);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
