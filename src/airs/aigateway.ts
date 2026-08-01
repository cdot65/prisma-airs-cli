import { AIGatewayClient, type AIGatewayClientOptions } from '@cdot65/prisma-airs-sdk';
import type {
  AiGatewayService,
  AiGatewayWorkspace,
  AiGatewayWorkspaceDetail,
  AiGatewayWorkspaceGetOptions,
  AiGatewayWorkspaceListOptions,
} from './types.js';

/**
 * `usage_limits`/`rate_limits` are `array | record | null` on the wire — the
 * array of policy objects is canonical, but the legacy single-object form is
 * still accepted upstream. Normalize everything to an array.
 */
function toLimitArray(value: unknown): Array<Record<string, unknown>> {
  if (Array.isArray(value)) return value as Array<Record<string, unknown>>;
  if (value !== null && typeof value === 'object') return [value as Record<string, unknown>];
  return [];
}

/** Normalize an SDK workspace list row into an AiGatewayWorkspace. */
function normalizeWorkspace(raw: Record<string, unknown>): AiGatewayWorkspace {
  return {
    id: raw.id as string,
    slug: raw.slug as string,
    name: raw.name as string,
    icon: raw.icon as string | null | undefined,
    description: raw.description as string | null | undefined,
    createdAt: raw.created_at as string | undefined,
    lastUpdatedAt: raw.last_updated_at as string | undefined,
    isDefault: Boolean(raw.is_default),
    status: raw.status as string | null | undefined,
    scopeName: raw.scope_name as string | undefined,
  };
}

/** Normalize an SDK workspace detail into an AiGatewayWorkspaceDetail. */
function normalizeWorkspaceDetail(raw: Record<string, unknown>): AiGatewayWorkspaceDetail {
  return {
    ...normalizeWorkspace(raw),
    defaults: raw.defaults as Record<string, unknown> | null | undefined,
    usageLimits: toLimitArray(raw.usage_limits),
    rateLimits: toLimitArray(raw.rate_limits),
    securitySettings: raw.security_settings as Record<string, boolean> | undefined,
    dataPlaneSecuritySettings: raw.data_plane_security_settings as
      | Record<string, unknown>
      | undefined,
    settings: raw.settings as Record<string, unknown> | undefined,
  };
}

/**
 * A 403 from the AI Gateway is a grant problem, and which grant depends on the
 * plane: `errorCode AB03` means the workspace-scope grant is missing (data
 * plane); otherwise the tenant-root admin grant is (admin plane). Returns a
 * user-facing hint, or undefined for non-403 errors.
 */
export function aiGatewayGrantHint(err: unknown): string | undefined {
  const status =
    (err as { status?: number; statusCode?: number })?.status ??
    (err as { statusCode?: number })?.statusCode;
  if (status !== 403) return undefined;
  const message = err instanceof Error ? err.message : String(err);
  const grant = message.includes('AB03')
    ? 'the service account is missing a workspace-scope grant (data plane, /ai_gw/v2)'
    : 'the service account is missing a tenant-root admin grant (admin plane, /ai_gw/admin/v2)';
  return (
    `${grant}. SCM Access Management edits the existing role row by default — ` +
    `use "Add Role" so the account ends up with both role rows, not one row moved.`
  );
}

/** Service wrapper over the SDK AIGatewayClient. */
export class SdkAiGatewayService implements AiGatewayService {
  private client: AIGatewayClient;

  constructor(opts?: AIGatewayClientOptions) {
    this.client = new AIGatewayClient(opts);
  }

  async listWorkspaces(options?: AiGatewayWorkspaceListOptions): Promise<AiGatewayWorkspace[]> {
    const response = await this.client.workspaces.list(options);
    return (response.data as Array<Record<string, unknown>>).map(normalizeWorkspace);
  }

  async listAllWorkspaces(): Promise<AiGatewayWorkspace[]> {
    // No single call returns both states: the API filters to active unless
    // asked otherwise, and only the admin plane sees the whole tenant.
    const [active, archived] = await Promise.all([
      this.client.workspaces.list({ plane: 'admin' }),
      this.client.workspaces.list({ plane: 'admin', status: 'archived' }),
    ]);
    return [
      ...(active.data as Array<Record<string, unknown>>),
      ...(archived.data as Array<Record<string, unknown>>),
    ].map(normalizeWorkspace);
  }

  async getWorkspace(
    workspaceRef: string,
    options?: AiGatewayWorkspaceGetOptions,
  ): Promise<AiGatewayWorkspaceDetail> {
    const raw = (await this.client.workspaces.get(workspaceRef, options)) as Record<
      string,
      unknown
    >;
    return normalizeWorkspaceDetail(raw);
  }
}
