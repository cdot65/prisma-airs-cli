import type { SecurityProfile } from '@cdot65/prisma-airs-sdk';

type ObjectValue = Record<string, unknown>;
function object(value: unknown): value is ObjectValue {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** Compare requested policy with read-back. Only observed, omitted server fields may differ.
 * Explicit source values (including null), unknown additions and enforcement changes fail.
 * This belongs in transfer verification, never in SDK parsing or request serialization.
 */
export function compareRuntimePolicies(
  expected: SecurityProfile['policy'],
  actual: SecurityProfile['policy'],
): { matches: boolean; serverDefaults: string[]; differences: string[] } {
  const serverDefaults: string[] = [];
  const differences: string[] = [];
  const visit = (
    source: unknown,
    target: unknown,
    path: string,
    insideToxicCategories = false,
    insideTopicGuardrails = false,
  ): void => {
    if (source === target) return;
    if (Array.isArray(source) && Array.isArray(target)) {
      if (source.length !== target.length) differences.push(`${path}.length`);
      source.forEach((item, index) => {
        visit(
          item,
          target[index],
          `${path}[${index}]`,
          insideToxicCategories,
          insideTopicGuardrails,
        );
      });
      return;
    }
    if (object(source) && object(target)) {
      for (const key of new Set([...Object.keys(source), ...Object.keys(target)])) {
        const field = `${path}.${key}`;
        if (!Object.hasOwn(source, key) && Object.hasOwn(target, key)) {
          const model = /^policy\.ai-security-profiles\[\d+\]\.model-configuration/;
          const relative = path.replace(model, 'model');
          let allowed: unknown;
          if (relative === 'model.data-protection' && key === 'database-security') allowed = null;
          if (key === 'severity') {
            if (/^model\.data-protection\.database-security\[\d+\]$/.test(relative)) {
              const values: Record<string, string> = {
                'database-security-create': 'medium',
                'database-security-read': 'low',
                'database-security-update': 'medium',
                'database-security-delete': 'high',
              };
              if (source.name === target.name && typeof source.name === 'string')
                allowed = Object.hasOwn(values, source.name) ? values[source.name] : undefined;
            }
            if (relative === 'model.data-protection.source-code-detection') allowed = 'high';
            if (
              relative === 'model.app-protection.malicious-code-protection' &&
              source.name === 'malicious-code' &&
              target.name === source.name
            )
              allowed = 'high';
            if (
              /^model\.model-protection\[\d+\]$/.test(relative) &&
              (source.name === 'prompt-injection' ||
                source.name === 'topic-guardrails' ||
                (source.name === 'contextual-grounding' && source.action === 'block')) &&
              target.name === source.name
            )
              allowed = 'medium';
            if (
              insideTopicGuardrails &&
              /^model\.model-protection\[\d+\]\.topic-list\[\d+\]\.topic\[\d+\]$/.test(relative) &&
              typeof source.topic_id === 'string' &&
              source.topic_id.length > 0 &&
              source.topic_id === target.topic_id &&
              source.topic_name === target.topic_name &&
              Number.isInteger(source.revision) &&
              source.revision === target.revision
            )
              allowed = 'medium';
            if (
              /^model\.agent-protection\[\d+\]$/.test(relative) &&
              source.name === 'agent-security' &&
              target.name === source.name
            )
              allowed = 'medium';
          }
          if (relative === 'model.app-protection' && key === 'url-detected-severity')
            allowed = 'low';
          if (
            relative === 'model.app-protection' &&
            key === 'default-url-category' &&
            object(target[key])
          ) {
            const value = target[key];
            if (Object.keys(value).length === 1 && value.member === null) {
              serverDefaults.push(field);
              continue;
            }
          }
          if (
            ((/^model\.model-protection\[\d+\]$/.test(relative) &&
              source.name === 'toxic-content' &&
              target.name === source.name) ||
              (insideToxicCategories &&
                /^model\.model-protection\[\d+\]\.toxic-category-list\[\d+\]$/.test(relative) &&
                typeof source.category === 'string' &&
                source.category.length > 0 &&
                source.category === target.category)) &&
            key === 'severity-by-confidence' &&
            object(target[key])
          ) {
            const value = target[key];
            if (
              Object.keys(value).length === 2 &&
              value.high === 'medium' &&
              value.moderate === 'low'
            ) {
              serverDefaults.push(field);
              continue;
            }
          }
          if (allowed !== undefined && target[key] === allowed) {
            serverDefaults.push(field);
            continue;
          }
          differences.push(field);
        } else if (!Object.hasOwn(target, key)) differences.push(field);
        else
          visit(
            source[key],
            target[key],
            field,
            key === 'toxic-category-list' &&
              source.name === 'toxic-content' &&
              target.name === source.name &&
              /^policy\.ai-security-profiles\[\d+\]\.model-configuration\.model-protection\[\d+\]$/.test(
                path,
              ),
            (key === 'topic-list' &&
              source.name === 'topic-guardrails' &&
              target.name === source.name &&
              /^policy\.ai-security-profiles\[\d+\]\.model-configuration\.model-protection\[\d+\]$/.test(
                path,
              )) ||
              (insideTopicGuardrails &&
                key === 'topic' &&
                source.action === 'block' &&
                target.action === 'block'),
          );
      }
      return;
    }
    differences.push(path);
  };
  visit(expected, actual, 'policy');
  return { matches: differences.length === 0, serverDefaults, differences };
}
