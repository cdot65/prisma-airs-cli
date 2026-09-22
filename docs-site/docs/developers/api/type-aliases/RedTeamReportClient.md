# Type Alias: RedTeamReportClient

> **RedTeamReportClient** = `Pick`\<`RedTeamClient`, `"getDashboardOverview"` \| `"getScanStatistics"`\> & `object`

Defined in: src/reports/redteam.ts:5

Read-only SDK surface; older SDK installations explicitly report quota as unavailable.

## Type Declaration

### adapters

> **adapters**: `Pick`\<`RedTeamClient`\[`"adapters"`\], `"list"`\>

### getQuotaSummary?

> `optional` **getQuotaSummary?**: () => `Promise`\<`QuotaSummary`\>

#### Returns

`Promise`\<`QuotaSummary`\>

### networkBroker

> **networkBroker**: `Pick`\<`RedTeamClient`\[`"networkBroker"`\], `"getChannelStats"`\>

### scans

> **scans**: `Pick`\<`RedTeamClient`\[`"scans"`\], `"list"`\>

### targets

> **targets**: `Pick`\<`RedTeamClient`\[`"targets"`\], `"list"`\>
