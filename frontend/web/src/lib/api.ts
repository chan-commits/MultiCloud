export type Organization = { id: string; slug: string; name: string; status: string };
export type ProviderAccount = {
  id: string;
  provider_kind: 'cloudflare' | 'vultr' | 'ovh' | string;
  name: string;
  status: string;
  capabilities: string[];
  last_validated_at: string | null;
  last_error_code: string | null;
  credential_type: string | null;
  credential_risk_level: string | null;
  credential_masked_identifier: string | null;
  created_at: string;
};
export type ResourceState = { version: number; state: Record<string, unknown> };
export type Resource = {
  id: string;
  resource_type: string;
  name: string;
  lifecycle: string;
  region: string | null;
  attributes: Record<string, unknown>;
  desired_state: ResourceState | null;
  observed_state: ResourceState | null;
  provider_account_id: string | null;
  provider_kind: string | null;
  external_id: string | null;
};
export type Operation = {
  id: string;
  operation_type: string;
  target_type: string;
  target_id: string | null;
  status: string;
  progress: number;
  error_code: string | null;
  error_message: string | null;
  created_at: string;
};
export type Drift = {
  id: string;
  fingerprint: string;
  status: string;
  differences: Record<string, unknown>;
  detected_at: string;
};
export type Reconciliation = {
  id: string;
  drift_id: string;
  policy: string;
  status: string;
  desired_version: number;
  operation_id: string | null;
};
export type AuditLog = {
  id: string;
  source_event_id: string;
  actor_type: string;
  actor_id: string | null;
  action: string;
  target_type: string;
  target_id: string;
  outcome: string;
  severity: string;
  trace_id: string | null;
  changes: Record<string, unknown>;
  metadata: Record<string, unknown>;
  occurred_at: string;
};
export type AuditFilters = {
  action?: string;
  target_type?: string;
  outcome?: string;
  occurred_before?: string;
  occurred_before_id?: string;
  limit?: number;
};
export type RegistrationSettings = { initialized: boolean; registration_enabled: boolean };
export type LoggingSettings = { log_level: 'error' | 'warn' | 'info' | 'debug' | 'trace' };

const API_ROOT = '/api/v1';

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

export class ApiClient {
  constructor(
    private token: string,
    private organizationId = '',
  ) {}
  setOrganization(id: string) {
    this.organizationId = id;
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const headers = new Headers(init.headers);
    headers.set('Authorization', `Bearer ${this.token}`);
    if (this.organizationId) headers.set('x-organization-id', this.organizationId);
    if (init.body) headers.set('Content-Type', 'application/json');
    const response = await fetch(`${API_ROOT}${path}`, { ...init, headers });
    if (!response.ok) {
      let message = `Request failed (${response.status})`;
      try {
        const body = (await response.json()) as { error?: string; message?: string };
        message = body.message ?? body.error ?? message;
      } catch {
        /* Preserve the useful HTTP status for non-JSON upstream errors. */
      }
      throw new ApiError(message, response.status);
    }
    if (response.status === 204) return undefined as T;
    return (await response.json()) as T;
  }

  organizations() {
    return this.request<Organization[]>('/organizations/');
  }
  logout() {
    return this.request<void>('/auth/logout', { method: 'POST' });
  }
  updateRegistration(registrationEnabled: boolean) {
    return this.request<RegistrationSettings>('/auth/registration-settings', {
      method: 'PUT',
      body: JSON.stringify({ registration_enabled: registrationEnabled }),
    });
  }
  loggingSettings() {
    return this.request<LoggingSettings>('/platform/settings/logging');
  }
  updateLogLevel(logLevel: LoggingSettings['log_level']) {
    return this.request<LoggingSettings>('/platform/settings/logging', {
      method: 'PUT',
      body: JSON.stringify({ log_level: logLevel }),
    });
  }
  createOrganization(payload: { name: string; slug: string }) {
    return this.request<Organization>('/organizations/', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  }
  providers() {
    return this.request<ProviderAccount[]>('/providers/');
  }
  resources() {
    return this.request<Resource[]>('/resources/');
  }
  operations() {
    return this.request<Operation[]>('/operations/');
  }
  auditLogs(filters: AuditFilters = {}) {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(filters)) if (value) query.set(key, String(value));
    query.set('limit', String(filters.limit ?? 100));
    return this.request<AuditLog[]>(`/audit-logs/?${query}`);
  }
  async downloadAudit(filters: AuditFilters = {}) {
    const headers = new Headers({ Authorization: `Bearer ${this.token}` });
    headers.set('x-organization-id', this.organizationId);
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(filters)) if (value) query.set(key, String(value));
    const response = await fetch(`${API_ROOT}/audit-logs/export?${query}`, { headers });
    if (!response.ok)
      throw new ApiError(`Audit export failed (${response.status})`, response.status);
    const url = URL.createObjectURL(await response.blob());
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'multicloud-audit.csv';
    anchor.click();
    URL.revokeObjectURL(url);
  }
  createProvider(payload: Record<string, unknown>) {
    return this.request<ProviderAccount>('/providers/', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  }
  testProvider(id: string) {
    return this.request<{ valid: boolean; capabilities: string[]; error_code: string | null }>(
      `/providers/${id}/connection-test`,
      { method: 'POST' },
    );
  }
  syncProvider(id: string, resourceType: string, parentExternalId: string | null = null) {
    return this.request<{ operation_id: string; status: string }>(`/providers/${id}/sync`, {
      method: 'POST',
      body: JSON.stringify({
        resource_type: resourceType,
        parent_external_id: parentExternalId,
        cursor: null,
        idempotency_key: crypto.randomUUID(),
      }),
    });
  }
  runProviderOperation(providerId: string, action: string, externalId: string) {
    return this.request<{ operation_id: string; status: string }>(
      `/providers/${providerId}/operations`,
      {
        method: 'POST',
        body: JSON.stringify({
          action,
          resource_type: 'compute_instance',
          external_id: externalId,
          parameters: {},
          idempotency_key: crypto.randomUUID(),
        }),
      },
    );
  }
  cancelOperation(id: string) {
    return this.request<Operation>(`/operations/${id}/cancel`, { method: 'POST' });
  }
  drifts(resourceId: string) {
    return this.request<Drift[]>(`/resources/${resourceId}/drifts`);
  }
  reconciliations(resourceId: string) {
    return this.request<Reconciliation[]>(`/resources/${resourceId}/reconciliations`);
  }
  approveReconciliation(resourceId: string, taskId: string) {
    return this.request<Reconciliation>(
      `/resources/${resourceId}/reconciliations/${taskId}/approve`,
      { method: 'POST' },
    );
  }
}

export async function login(email: string, password: string) {
  const response = await fetch(`${API_ROOT}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });
  if (!response.ok) throw new ApiError('Email or password is incorrect', response.status);
  return (await response.json()) as {
    access_token: string;
    expires_at: string;
    is_platform_admin: boolean;
  };
}

export async function registrationSettings() {
  const response = await fetch(`${API_ROOT}/auth/registration-settings`);
  if (!response.ok) throw new ApiError('Could not load registration settings', response.status);
  return (await response.json()) as RegistrationSettings;
}

export async function register(email: string, password: string, displayName: string) {
  const response = await fetch(`${API_ROOT}/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password, display_name: displayName }),
  });
  if (!response.ok) {
    let message = `Registration failed (${response.status})`;
    try {
      message = ((await response.json()) as { message?: string }).message ?? message;
    } catch {
      /* use status */
    }
    throw new ApiError(message, response.status);
  }
}
