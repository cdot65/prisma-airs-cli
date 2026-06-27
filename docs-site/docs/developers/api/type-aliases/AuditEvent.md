# Type Alias: AuditEvent

> **AuditEvent** = \{ `topics`: [`ProfileTopic`](../interfaces/ProfileTopic.md)[]; `type`: `"topics:loaded"`; \} \| \{ `count`: `number`; `topicName`: `string`; `type`: `"tests:generated"`; \} \| \{ `completed`: `number`; `total`: `number`; `type`: `"scan:progress"`; \} \| \{ `result`: [`AuditResult`](../interfaces/AuditResult.md); `type`: `"evaluate:complete"`; \} \| \{ `result`: [`AuditResult`](../interfaces/AuditResult.md); `type`: `"audit:complete"`; \}

Defined in: [src/audit/types.ts:41](https://github.com/cdot65/prisma-airs-cli/blob/main/src/audit/types.ts#L41)

Audit event union yielded by the audit runner.
