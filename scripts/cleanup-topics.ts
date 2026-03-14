import 'dotenv/config';
import { ManagementClient } from '@cdot65/prisma-airs-sdk';

async function main() {
  const mgmt = new ManagementClient();
  const { custom_topics } = await mgmt.topics.list();

  const weaponsTopics = custom_topics.filter(
    (t) => t.topic_name === 'Weapons Manufacturing and Procurement',
  );

  console.log(`Found ${weaponsTopics.length} weapons topic(s) to clean up`);
  for (const t of weaponsTopics) {
    console.log(`  Deleting ${t.topic_id} (${t.topic_name})...`);
    try {
      if (!t.topic_id) continue;
      await mgmt.topics.forceDelete(t.topic_id);
      console.log('  Done');
    } catch (err) {
      console.error('  Failed:', err);
    }
  }
}

main().catch(console.error);
