import 'dotenv/config';
import { ManagementClient } from '@cdot65/prisma-airs-sdk';

async function main() {
  const mgmt = new ManagementClient();

  console.log('=== Security Profiles ===');
  const { ai_profiles } = await mgmt.profiles.list();
  for (const p of ai_profiles) {
    console.log(`\n[${p.profile_id}] ${p.profile_name} (active: ${p.active})`);
    console.log('  Full policy:', JSON.stringify(p.policy, null, 2));
    // Also dump everything else
    const { policy, ...rest } = p;
    console.log('  Other fields:', JSON.stringify(rest, null, 2));
  }

  // Also try updating a topic with explicit active: true
  console.log('\n=== Updating topic with active: true ===');
  const { custom_topics } = await mgmt.topics.list();
  const weaponsTopic = custom_topics.find(
    (t) => t.topic_name === 'Weapons Manufacturing and Procurement',
  );
  if (weaponsTopic?.topic_id) {
    const updated = await mgmt.topics.update(weaponsTopic.topic_id, {
      topic_name: weaponsTopic.topic_name,
      description: weaponsTopic.description,
      examples: weaponsTopic.examples,
      active: true,
    });
    console.log('  Updated:', JSON.stringify(updated, null, 2));
  }
}

main().catch(console.error);
