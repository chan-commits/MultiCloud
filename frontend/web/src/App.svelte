<script lang="ts">
  import { onMount } from 'svelte';
  import AuthScreen from './components/AuthScreen.svelte';
  import OrganizationOnboarding from './components/OrganizationOnboarding.svelte';
  import {
    ApiClient,
    ApiError,
    login,
    register,
    registrationSettings,
    type AuditFilters,
    type AuditLog,
    type Drift,
    type Operation,
    type Organization,
    type ProviderAccount,
    type Reconciliation,
    type Resource,
  } from './lib/api';

  type View = 'overview' | 'providers' | 'resources' | 'operations' | 'audit';
  type ProviderKind = 'cloudflare' | 'vultr' | 'ovh';
  const navigation: { id: View; label: string; caption: string; icon: string }[] = [
    { id: 'overview', label: 'Command Center', caption: 'Global posture', icon: '◫' },
    { id: 'providers', label: 'Provider Fabric', caption: 'Connections', icon: '⌁' },
    { id: 'resources', label: 'Resource Matrix', caption: 'Live inventory', icon: '◇' },
    { id: 'operations', label: 'Operation Stream', caption: 'Execution trace', icon: '↯' },
    { id: 'audit', label: 'Audit Stream', caption: 'Immutable trail', icon: '≋' },
  ];

  let token = $state(''),
    loginError = $state(''),
    error = $state(''),
    notice = $state('');
  let registrationEnabled = $state(false),
    platformInitialized = $state(false),
    isPlatformAdmin = $state(false),
    registrationBusy = $state(false);
  let loading = $state(false),
    authenticating = $state(false),
    mobileNav = $state(false),
    providerDialog = $state(false),
    savingProvider = $state(false);
  let organizations = $state<Organization[]>([]),
    organizationId = $state(''),
    providers = $state<ProviderAccount[]>([]),
    resources = $state<Resource[]>([]),
    operations = $state<Operation[]>([]),
    auditLogs = $state<AuditLog[]>([]);
  let view = $state<View>('overview');
  let providerKind = $state<ProviderKind>('cloudflare'),
    providerName = $state(''),
    apiToken = $state(''),
    emailIdentity = $state(''),
    globalApiKey = $state('');
  let applicationKey = $state(''),
    applicationSecret = $state(''),
    consumerKey = $state(''),
    useGlobalKey = $state(false),
    actionBusy = $state('');
  let selectedResource = $state<Resource | null>(null),
    resourceDrifts = $state<Drift[]>([]),
    reconciliations = $state<Reconciliation[]>([]),
    detailLoading = $state(false);
  let creatingOrganization = $state(false);
  let auditAction = $state(''),
    auditOutcome = $state(''),
    auditLoadingMore = $state(false),
    auditHasMore = $state(false);
  let client = $state<ApiClient | null>(null);

  let activeOrganization = $derived(organizations.find((item) => item.id === organizationId));
  let activeResources = $derived(resources.filter((item) => item.lifecycle === 'active'));
  let driftedResources = $derived(
    resources.filter(
      (item) =>
        item.desired_state &&
        item.observed_state &&
        JSON.stringify(item.desired_state.state) !== JSON.stringify(item.observed_state.state),
    ),
  );
  let runningOperations = $derived(
    operations.filter((item) => ['queued', 'running', 'retrying'].includes(item.status)),
  );
  let failedOperations = $derived(operations.filter((item) => item.status === 'failed'));

  onMount(async () => {
    try {
      const settings = await registrationSettings();
      registrationEnabled = settings.registration_enabled;
      platformInitialized = settings.initialized;
    } catch {
      loginError = 'Could not load platform registration status.';
    }
    const stored = sessionStorage.getItem('multicloud.session');
    if (!stored) return;
    try {
      const session = JSON.parse(stored) as {
        token: string;
        expiresAt: string;
        isPlatformAdmin?: boolean;
      };
      if (new Date(session.expiresAt) <= new Date()) throw new Error('expired');
      token = session.token;
      isPlatformAdmin = session.isPlatformAdmin ?? false;
      await initialize();
    } catch {
      sessionStorage.removeItem('multicloud.session');
      token = '';
    }
  });

  async function submitLogin(email: string, password: string) {
    authenticating = true;
    loginError = '';
    try {
      const session = await login(email, password);
      token = session.access_token;
      isPlatformAdmin = session.is_platform_admin;
      sessionStorage.setItem(
        'multicloud.session',
        JSON.stringify({
          token,
          expiresAt: session.expires_at,
          isPlatformAdmin: session.is_platform_admin,
        }),
      );
      await initialize();
    } catch (cause) {
      loginError = messageOf(cause);
      token = '';
    } finally {
      authenticating = false;
    }
  }

  async function submitRegistration(email: string, password: string, displayName: string) {
    authenticating = true;
    loginError = '';
    try {
      await register(email, password, displayName);
      const session = await login(email, password);
      token = session.access_token;
      isPlatformAdmin = session.is_platform_admin;
      sessionStorage.setItem(
        'multicloud.session',
        JSON.stringify({
          token,
          expiresAt: session.expires_at,
          isPlatformAdmin: session.is_platform_admin,
        }),
      );
      await initialize();
    } catch (cause) {
      loginError = messageOf(cause);
      token = '';
    } finally {
      authenticating = false;
    }
  }

  async function initialize() {
    loading = true;
    error = '';
    client = new ApiClient(token);
    try {
      organizations = await client.organizations();
      const preferred = localStorage.getItem('multicloud.organization');
      organizationId = organizations.some((item) => item.id === preferred)
        ? (preferred ?? '')
        : (organizations[0]?.id ?? '');
      if (organizationId) {
        client.setOrganization(organizationId);
        await refreshAll();
      }
    } catch (cause) {
      if (cause instanceof ApiError && cause.status === 401) logout();
      else error = messageOf(cause);
    } finally {
      loading = false;
    }
  }

  async function changeOrganization() {
    if (!client || !organizationId) return;
    localStorage.setItem('multicloud.organization', organizationId);
    client.setOrganization(organizationId);
    selectedResource = null;
    await refreshAll();
  }
  async function refreshAll() {
    if (!client || !organizationId) return;
    loading = true;
    error = '';
    try {
      [providers, resources, operations, auditLogs] = await Promise.all([
        client.providers(),
        client.resources(),
        client.operations(),
        client.auditLogs(auditFilters()),
      ]);
      auditHasMore = auditLogs.length === 100;
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      loading = false;
    }
  }
  async function createOrganization(organizationName: string, organizationSlug: string) {
    if (!client) return;
    creatingOrganization = true;
    error = '';
    try {
      const organization = await client.createOrganization({
        name: organizationName,
        slug: organizationSlug,
      });
      organizations = [...organizations, organization];
      organizationId = organization.id;
      client.setOrganization(organization.id);
      localStorage.setItem('multicloud.organization', organization.id);
      notice = 'Organization workspace created.';
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      creatingOrganization = false;
    }
  }
  function auditFilters(before?: AuditLog): AuditFilters {
    return {
      action: auditAction.trim() || undefined,
      outcome: auditOutcome || undefined,
      occurred_before: before?.occurred_at,
      occurred_before_id: before?.id,
      limit: 100,
    };
  }
  async function applyAuditFilters() {
    if (!client) return;
    loading = true;
    error = '';
    try {
      auditLogs = await client.auditLogs(auditFilters());
      auditHasMore = auditLogs.length === 100;
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      loading = false;
    }
  }
  async function loadMoreAudit() {
    if (!client || !auditLogs.length) return;
    auditLoadingMore = true;
    try {
      const rows = await client.auditLogs(auditFilters(auditLogs.at(-1)));
      const existing = new Set(auditLogs.map((item) => item.id));
      auditLogs = [...auditLogs, ...rows.filter((item) => !existing.has(item.id))];
      auditHasMore = rows.length === 100;
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      auditLoadingMore = false;
    }
  }
  async function logout() {
    try {
      await client?.logout();
    } catch {
      /* Local sign-out must still complete. */
    }
    sessionStorage.removeItem('multicloud.session');
    localStorage.removeItem('multicloud.organization');
    token = '';
    isPlatformAdmin = false;
    client = null;
    organizations = [];
    providers = [];
    resources = [];
    operations = [];
    auditLogs = [];
  }

  async function toggleRegistration() {
    if (!client || !organizationId || !isPlatformAdmin) return;
    registrationBusy = true;
    try {
      const settings = await client.updateRegistration(!registrationEnabled);
      registrationEnabled = settings.registration_enabled;
      notice = `Public registration ${registrationEnabled ? 'enabled' : 'disabled'}.`;
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      registrationBusy = false;
    }
  }

  async function createProvider() {
    if (!client) return;
    savingProvider = true;
    error = '';
    try {
      let credential: Record<string, unknown>;
      if (providerKind === 'cloudflare' && useGlobalKey)
        credential = {
          credential_type: 'global_api_key',
          email: emailIdentity,
          global_api_key: globalApiKey,
        };
      else if (providerKind === 'ovh')
        credential = {
          credential_type: 'ovh_application',
          application_key: applicationKey,
          application_secret: applicationSecret,
          consumer_key: consumerKey,
        };
      else credential = { credential_type: 'api_token', api_token: apiToken };
      await client.createProvider({
        provider_kind: providerKind,
        name: providerName,
        configuration: {},
        ...credential,
      });
      providerDialog = false;
      clearCredentialForm();
      notice = 'Provider account encrypted and ready for validation.';
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      savingProvider = false;
    }
  }
  function clearCredentialForm() {
    providerName = '';
    apiToken = '';
    emailIdentity = '';
    globalApiKey = '';
    applicationKey = '';
    applicationSecret = '';
    consumerKey = '';
    useGlobalKey = false;
  }

  async function testConnection(provider: ProviderAccount) {
    if (!client) return;
    actionBusy = provider.id;
    try {
      const result = await client.testProvider(provider.id);
      notice = result.valid
        ? `${provider.name} connection verified.`
        : `${provider.name} validation failed.`;
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
  }
  async function syncProvider(provider: ProviderAccount) {
    if (!client) return;
    actionBusy = provider.id;
    try {
      await client.syncProvider(
        provider.id,
        provider.provider_kind === 'cloudflare' ? 'dns_zone' : 'compute_instance',
      );
      notice = `${provider.name} inventory sync queued.`;
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
  }
  async function openResource(resource: Resource) {
    if (!client) return;
    selectedResource = resource;
    detailLoading = true;
    try {
      [resourceDrifts, reconciliations] = await Promise.all([
        client.drifts(resource.id),
        client.reconciliations(resource.id),
      ]);
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      detailLoading = false;
    }
  }
  function resourceProvider(resource: Resource) {
    return providers.find((item) => item.id === resource.provider_account_id) ?? null;
  }
  async function lifecycle(resource: Resource, action: string) {
    if (!client) return;
    const provider = resourceProvider(resource),
      externalId = resource.external_id ?? '';
    if (!provider || !externalId) {
      error = 'This resource does not expose an operable provider mapping yet.';
      return;
    }
    actionBusy = `${resource.id}:${action}`;
    try {
      await client.runProviderOperation(provider.id, action, externalId);
      notice = `${action} operation queued for ${resource.name}.`;
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
  }
  async function approve(task: Reconciliation) {
    if (!client || !selectedResource) return;
    actionBusy = task.id;
    try {
      await client.approveReconciliation(selectedResource.id, task.id);
      notice = 'Reconciliation approved.';
      await openResource(selectedResource);
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
  }
  async function cancel(operation: Operation) {
    if (!client) return;
    actionBusy = operation.id;
    try {
      await client.cancelOperation(operation.id);
      notice = 'Queued operation cancelled.';
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
  }
  async function exportAudit() {
    if (!client) return;
    actionBusy = 'audit-export';
    try {
      await client.downloadAudit(auditFilters());
      notice = 'Sanitized audit export generated.';
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
  }
  function messageOf(cause: unknown) {
    return cause instanceof Error ? cause.message : 'An unexpected error occurred';
  }
  function relativeDate(value: string | null) {
    if (!value) return 'Never';
    const seconds = Math.floor((Date.now() - new Date(value).getTime()) / 1000);
    if (seconds < 60) return 'Just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric' }).format(
      new Date(value),
    );
  }
  function shortId(value: string) {
    return value.length > 16 ? `${value.slice(0, 8)}…${value.slice(-4)}` : value;
  }
</script>

<svelte:head
  ><title>MultiCloud · Command Center</title><meta
    name="description"
    content="Multi-tenant cloud operations control plane"
  /></svelte:head
>

{#if !token}
  <AuthScreen
    {registrationEnabled}
    {platformInitialized}
    {authenticating}
    error={loginError}
    onLogin={submitLogin}
    onRegister={submitRegistration}
  />
{:else}
  <div class="app-shell">
    <aside class:open={mobileNav}>
      <div class="brand sidebar-brand">
        <span class="brand-glyph">M</span><span>MultiCloud</span>
      </div>
      <div class="tenant-chip">
        <span class="tenant-avatar"
          >{activeOrganization?.name.slice(0, 2).toUpperCase() ?? '--'}</span
        >
        <div>
          <small>ACTIVE ORGANIZATION</small><strong
            >{activeOrganization?.name ?? 'Select tenant'}</strong
          >
        </div>
      </div>
      <nav aria-label="Primary navigation">
        {#each navigation as item}<button
            class:active={view === item.id}
            onclick={() => {
              view = item.id;
              mobileNav = false;
            }}
            ><span class="nav-icon">{item.icon}</span><span
              ><strong>{item.label}</strong><small>{item.caption}</small></span
            ></button
          >{/each}
      </nav>
      <div class="sidebar-foot">
        <div class="system-health">
          <i></i><span><strong>Control plane</strong><small>All systems nominal</small></span>
        </div>
        <button class="text-button" onclick={logout}>Sign out</button>
      </div>
    </aside>
    <main class="workspace">
      <header class="topbar">
        <button
          class="menu-button"
          aria-label="Toggle navigation"
          onclick={() => (mobileNav = !mobileNav)}>☰</button
        >
        <div>
          <p class="breadcrumb">CONTROL PLANE / <span>{view.toUpperCase()}</span></p>
          <h1>{navigation.find((item) => item.id === view)?.label}</h1>
        </div>
        <div class="top-actions">
          {#if isPlatformAdmin}<button
              class="registration-toggle"
              class:enabled={registrationEnabled}
              onclick={toggleRegistration}
              disabled={registrationBusy || !organizationId}
              title="Platform-wide public registration"
              ><i></i>{registrationEnabled ? 'Registration on' : 'Registration off'}</button
            >{/if}
          <select
            aria-label="Organization"
            bind:value={organizationId}
            onchange={changeOrganization}
            >{#each organizations as organization}<option value={organization.id}
                >{organization.name}</option
              >{/each}</select
          ><button
            class="icon-button"
            aria-label="Refresh data"
            onclick={refreshAll}
            disabled={loading}>↻</button
          ><span class="operator-avatar">OP</span>
        </div>
      </header>
      {#if error}<div class="alert error">
          <span>!</span>
          <p>{error}</p>
          <button onclick={() => (error = '')}>×</button>
        </div>{/if}{#if notice}<div class="alert success">
          <span>✓</span>
          <p>{notice}</p>
          <button onclick={() => (notice = '')}>×</button>
        </div>{/if}
      <div class="content" class:loading>
        {#if !organizationId}<OrganizationOnboarding
            creating={creatingOrganization}
            onCreate={createOrganization}
          />
        {:else if view === 'overview'}
          <section class="hero-panel">
            <div>
              <p class="eyebrow">LIVE INFRASTRUCTURE POSTURE</p>
              <h2>Your cloud estate,<br /><span>resolved in real time.</span></h2>
              <p>Unified visibility across every connected provider and canonical resource.</p>
            </div>
            <div class="pulse-core">
              <div class="pulse-ring"></div>
              <strong>{activeResources.length}</strong><small>ACTIVE</small>
            </div>
          </section>
          <section class="metric-grid">
            <article>
              <div class="metric-head"><span>CONNECTED PROVIDERS</span><i class="cyan">⌁</i></div>
              <strong
                >{providers.filter((item) => item.status === 'active').length}<small>
                  / {providers.length}</small
                ></strong
              >
              <p><span class="up">●</span> Capability registry online</p>
            </article>
            <article>
              <div class="metric-head"><span>MANAGED RESOURCES</span><i class="violet">◇</i></div>
              <strong>{resources.length}</strong>
              <p>{activeResources.length} currently active</p>
            </article>
            <article>
              <div class="metric-head"><span>ACTIVE OPERATIONS</span><i class="amber">↯</i></div>
              <strong>{runningOperations.length}</strong>
              <p>{failedOperations.length} failures in recent history</p>
            </article>
            <article>
              <div class="metric-head"><span>CONFIGURATION DRIFT</span><i class="rose">∆</i></div>
              <strong>{driftedResources.length}</strong>
              <p>{driftedResources.length ? 'Review required' : 'Desired state aligned'}</p>
            </article>
          </section>
          <section class="dashboard-grid">
            <article class="panel span-two">
              <div class="panel-head">
                <div>
                  <p class="kicker">OPERATION TELEMETRY</p>
                  <h3>Execution stream</h3>
                </div>
                <button class="text-button" onclick={() => (view = 'operations')}>View all →</button
                >
              </div>
              <div class="timeline-chart">
                <div class="chart-grid"></div>
                <svg viewBox="0 0 800 180" preserveAspectRatio="none"
                  ><path
                    d="M0 145 C70 130 90 135 150 105 S250 135 315 80 S420 115 485 55 S590 90 650 38 S750 55 800 20"
                  /></svg
                ><span>Time-series telemetry connects in Observability phase</span>
              </div>
            </article>
            <article class="panel">
              <div class="panel-head">
                <div>
                  <p class="kicker">PROVIDER FABRIC</p>
                  <h3>Connection status</h3>
                </div>
              </div>
              <div class="provider-mini-list">
                {#each providers.slice(0, 4) as provider}<button
                    onclick={() => (view = 'providers')}
                    ><span class="provider-logo {provider.provider_kind}"
                      >{provider.provider_kind.slice(0, 2).toUpperCase()}</span
                    ><span
                      ><strong>{provider.name}</strong><small>{provider.provider_kind}</small></span
                    ><i class:online={provider.status === 'active'}></i></button
                  >{:else}<div class="compact-empty">
                    Connect a provider to activate the fabric.
                  </div>{/each}
              </div>
            </article>
            <article class="panel span-two">
              <div class="panel-head">
                <div>
                  <p class="kicker">RECENT ACTIVITY</p>
                  <h3>Operation ledger</h3>
                </div>
              </div>
              <div class="table-wrap">
                <table>
                  <thead
                    ><tr
                      ><th>Operation</th><th>Target</th><th>Status</th><th>Progress</th><th
                        >Created</th
                      ></tr
                    ></thead
                  ><tbody
                    >{#each operations.slice(0, 5) as operation}<tr
                        ><td
                          ><strong>{operation.operation_type}</strong><small
                            >{shortId(operation.id)}</small
                          ></td
                        ><td>{operation.target_type}</td><td
                          ><span class="status {operation.status}">{operation.status}</span></td
                        ><td
                          ><div class="progress">
                            <i style={`width:${operation.progress}%`}></i>
                          </div></td
                        ><td>{relativeDate(operation.created_at)}</td></tr
                      >{:else}<tr
                        ><td colspan="5" class="table-empty">No operations recorded yet.</td></tr
                      >{/each}</tbody
                  >
                </table>
              </div>
            </article>
            <article class="panel">
              <div class="panel-head">
                <div>
                  <p class="kicker">COST SIGNAL</p>
                  <h3>Cloud spend</h3>
                </div>
                <span class="beta">PHASE 9</span>
              </div>
              <div class="future-metric">
                <span>—</span>
                <p>Billing telemetry is reserved for the Billing bounded context.</p>
                <div class="ghost-bars"><i></i><i></i><i></i><i></i><i></i><i></i></div>
              </div>
            </article>
          </section>
        {:else if view === 'providers'}
          <section class="page-intro">
            <div>
              <p class="eyebrow">ADAPTER REGISTRY</p>
              <h2>Provider Fabric</h2>
              <p>Encrypted credentials, capability discovery, and controlled synchronization.</p>
            </div>
            <button class="primary" onclick={() => (providerDialog = true)}
              >＋ Connect provider</button
            >
          </section>
          <section class="provider-grid">
            {#each providers as provider}<article class="provider-card">
                <div class="provider-card-top">
                  <span class="provider-logo large {provider.provider_kind}"
                    >{provider.provider_kind.slice(0, 2).toUpperCase()}</span
                  >
                  <div>
                    <p>{provider.provider_kind.toUpperCase()}</p>
                    <h3>{provider.name}</h3>
                  </div>
                  <span class="status {provider.status}">{provider.status}</span>
                </div>
                <div class="provider-data">
                  <div>
                    <small>CAPABILITIES</small>
                    <p>
                      {provider.capabilities.length
                        ? provider.capabilities.join(' · ')
                        : 'Awaiting discovery'}
                    </p>
                  </div>
                  <div>
                    <small>LAST VERIFIED</small>
                    <p>{relativeDate(provider.last_validated_at)}</p>
                  </div>
                  <div>
                    <small>CREDENTIAL</small>
                    <p>{provider.credential_masked_identifier ?? 'Encrypted'}</p>
                  </div>
                  <div>
                    <small>RISK</small>
                    <p class:high-risk={provider.credential_risk_level === 'high'}>
                      {provider.credential_risk_level ?? 'restricted'}
                    </p>
                  </div>
                </div>
                {#if provider.last_error_code}<p class="inline-warning">
                    ⚠ {provider.last_error_code}
                  </p>{/if}
                <div class="card-actions">
                  <button
                    onclick={() => testConnection(provider)}
                    disabled={actionBusy === provider.id}>Test connection</button
                  ><button
                    class="accent"
                    onclick={() => syncProvider(provider)}
                    disabled={provider.status !== 'active' || actionBusy === provider.id}
                    >{actionBusy === provider.id ? 'Working…' : 'Sync inventory'}</button
                  >
                </div>
              </article>{:else}<section class="empty-state panel-wide">
                <div class="empty-orbit">⌁</div>
                <h2>No providers connected</h2>
                <p>Connect Cloudflare, Vultr, or OVH to discover your first resources.</p>
                <button class="primary" onclick={() => (providerDialog = true)}
                  >Connect provider</button
                >
              </section>{/each}
          </section>
        {:else if view === 'resources'}
          <section class="page-intro">
            <div>
              <p class="eyebrow">CANONICAL INVENTORY</p>
              <h2>Resource Matrix</h2>
              <p>Provider-neutral assets with normalized state and drift visibility.</p>
            </div>
            <div class="segmented">
              <button class="active">All {resources.length}</button><button
                >Compute {resources.filter((item) => item.resource_type === 'compute_instance')
                  .length}</button
              ><button
                >DNS {resources.filter((item) => item.resource_type.startsWith('dns_'))
                  .length}</button
              >
            </div>
          </section>
          <section class="panel resource-panel">
            <div class="table-wrap">
              <table>
                <thead
                  ><tr
                    ><th>Resource</th><th>Type</th><th>Region</th><th>Lifecycle</th><th>Observed</th
                    ><th></th></tr
                  ></thead
                ><tbody
                  >{#each resources as resource}<tr
                      ><td
                        ><div class="resource-name">
                          <span>{resource.resource_type === 'compute_instance' ? '▣' : '◎'}</span>
                          <div>
                            <strong>{resource.name}</strong><small>{shortId(resource.id)}</small>
                          </div>
                        </div></td
                      ><td>{resource.resource_type.replaceAll('_', ' ')}</td><td
                        >{resource.region ?? 'global'}</td
                      ><td><span class="status {resource.lifecycle}">{resource.lifecycle}</span></td
                      ><td>v{resource.observed_state?.version ?? 0}</td><td
                        ><button class="row-action" onclick={() => openResource(resource)}
                          >Inspect →</button
                        ></td
                      ></tr
                    >{:else}<tr
                      ><td colspan="6"
                        ><div class="empty-row">
                          <span>◇</span><strong>No resources discovered</strong><small
                            >Run inventory sync from Provider Fabric.</small
                          >
                        </div></td
                      ></tr
                    >{/each}</tbody
                >
              </table>
            </div>
          </section>
        {:else if view === 'operations'}
          <section class="page-intro">
            <div>
              <p class="eyebrow">RELIABLE EXECUTION</p>
              <h2>Operation Stream</h2>
              <p>Idempotent commands, retry state, and immutable execution history.</p>
            </div>
            <div class="live-pill"><i></i> LIVE QUEUE</div>
          </section>
          <section class="panel operation-panel">
            <div class="table-wrap">
              <table>
                <thead
                  ><tr
                    ><th>ID / Type</th><th>Target</th><th>Status</th><th>Progress</th><th
                      >Created</th
                    ><th></th></tr
                  ></thead
                ><tbody
                  >{#each operations as operation}<tr
                      ><td
                        ><strong>{operation.operation_type}</strong><small
                          >{shortId(operation.id)}</small
                        ></td
                      ><td
                        ><strong>{operation.target_type}</strong><small
                          >{operation.target_id ? shortId(operation.target_id) : '—'}</small
                        ></td
                      ><td
                        ><span class="status {operation.status}">{operation.status}</span
                        >{#if operation.error_code}<small class="error-code"
                            >{operation.error_code}</small
                          >{/if}</td
                      ><td
                        ><div class="progress labeled">
                          <i style={`width:${operation.progress}%`}></i><span
                            >{operation.progress}%</span
                          >
                        </div></td
                      ><td>{relativeDate(operation.created_at)}</td><td
                        >{#if operation.status === 'queued'}<button
                            class="row-action danger"
                            onclick={() => cancel(operation)}
                            disabled={actionBusy === operation.id}>Cancel</button
                          >{/if}</td
                      ></tr
                    >{:else}<tr
                      ><td colspan="6" class="table-empty">Operation history is empty.</td></tr
                    >{/each}</tbody
                >
              </table>
            </div>
          </section>
        {:else}
          <section class="page-intro">
            <div>
              <p class="eyebrow">APPEND-ONLY SECURITY LEDGER</p>
              <h2>Audit Stream</h2>
              <p>
                Sanitized tenant events with actor, outcome, target, and immutable source identity.
              </p>
            </div>
            <button class="primary" onclick={exportAudit} disabled={actionBusy === 'audit-export'}
              >{actionBusy === 'audit-export' ? 'Generating…' : '↓ Export CSV'}</button
            >
          </section>
          <section class="audit-summary">
            <article><small>LOADED EVENTS</small><strong>{auditLogs.length}</strong></article>
            <article>
              <small>SECURITY WARNINGS</small><strong
                >{auditLogs.filter((item) => item.severity !== 'info').length}</strong
              >
            </article>
            <article>
              <small>FAILED OUTCOMES</small><strong
                >{auditLogs.filter((item) => item.outcome === 'failed' || item.outcome === 'denied')
                  .length}</strong
              >
            </article>
          </section>
          <form
            class="audit-filters"
            onsubmit={(event) => {
              event.preventDefault();
              applyAuditFilters();
            }}
          >
            <label
              >Action<input
                bind:value={auditAction}
                placeholder="provider.credential.updated"
              /></label
            ><label
              >Outcome<select bind:value={auditOutcome}
                ><option value="">All outcomes</option><option value="attempted">Attempted</option
                ><option value="succeeded">Succeeded</option><option value="failed">Failed</option
                ><option value="denied">Denied</option><option value="cancelled">Cancelled</option
                ></select
              ></label
            ><button>Apply filters</button>
          </form>
          <section class="panel operation-panel">
            <div class="table-wrap">
              <table>
                <thead
                  ><tr
                    ><th>Time / Event</th><th>Actor</th><th>Target</th><th>Outcome</th><th
                      >Severity</th
                    ><th>Trace</th></tr
                  ></thead
                ><tbody
                  >{#each auditLogs as audit}<tr
                      ><td
                        ><strong>{audit.action}</strong><small
                          >{relativeDate(audit.occurred_at)} · {shortId(
                            audit.source_event_id,
                          )}</small
                        ></td
                      ><td
                        ><strong>{audit.actor_type}</strong><small
                          >{audit.actor_id ? shortId(audit.actor_id) : 'control plane'}</small
                        ></td
                      ><td
                        ><strong>{audit.target_type}</strong><small
                          >{shortId(audit.target_id)}</small
                        ></td
                      ><td><span class="status {audit.outcome}">{audit.outcome}</span></td><td
                        ><span class="severity {audit.severity}">{audit.severity}</span></td
                      ><td>{audit.trace_id ? shortId(audit.trace_id) : '—'}</td></tr
                    >{:else}<tr
                      ><td colspan="6"
                        ><div class="empty-row">
                          <span>≋</span><strong>No projected audit events</strong><small
                            >New domain events appear after the Worker projection runs.</small
                          >
                        </div></td
                      ></tr
                    >{/each}</tbody
                >
              </table>
            </div>
            {#if auditHasMore}<button
                class="load-more"
                onclick={loadMoreAudit}
                disabled={auditLoadingMore}
                >{auditLoadingMore ? 'Loading…' : 'Load older events'}</button
              >{/if}
          </section>
        {/if}
      </div>
    </main>
  </div>

  {#if providerDialog}<div
      class="modal-backdrop"
      role="presentation"
      onclick={(event) => event.target === event.currentTarget && (providerDialog = false)}
    >
      <form
        class="modal"
        onsubmit={(event) => {
          event.preventDefault();
          createProvider();
        }}
      >
        <div class="modal-head">
          <div>
            <p class="kicker">NEW CONNECTION</p>
            <h2>Connect provider</h2>
          </div>
          <button type="button" onclick={() => (providerDialog = false)}>×</button>
        </div>
        <label
          >Provider<select bind:value={providerKind}
            ><option value="cloudflare">Cloudflare · DNS</option><option value="vultr"
              >Vultr · Compute</option
            ><option value="ovh">OVHcloud · VPS</option></select
          ></label
        ><label
          >Connection name<input
            bind:value={providerName}
            placeholder="Production account"
            required
          /></label
        >
        {#if providerKind === 'cloudflare'}<div class="credential-choice">
            <button
              type="button"
              class:active={!useGlobalKey}
              onclick={() => (useGlobalKey = false)}
              ><strong>API Token</strong><small>✓ Recommended · Restricted scope</small></button
            ><button type="button" class:risk={useGlobalKey} onclick={() => (useGlobalKey = true)}
              ><strong>Global API Key</strong><small>⚠ Legacy · Full account access</small></button
            >
          </div>
          {#if useGlobalKey}<div class="risk-banner">
              <strong>High-risk credential</strong><span
                >Use a scoped API Token whenever possible. This action is security-audited.</span
              >
            </div>
            <label>Cloudflare email<input bind:value={emailIdentity} type="email" required /></label
            ><label
              >Global API Key<input bind:value={globalApiKey} type="password" required /></label
            >{:else}<label
              >API Token<input
                bind:value={apiToken}
                type="password"
                autocomplete="off"
                required
              /><small>Token should include Zone Read and DNS Edit scopes.</small></label
            >{/if}
        {:else if providerKind === 'vultr'}<label
            >API Token<input
              bind:value={apiToken}
              type="password"
              autocomplete="off"
              required
            /><small>Use a dedicated token with minimum Compute permissions.</small></label
          >
        {:else}<div class="info-banner">
            OVHcloud signs each request with three encrypted credential components.
          </div>
          <label
            >Application Key<input bind:value={applicationKey} autocomplete="off" required /></label
          ><label
            >Application Secret<input
              bind:value={applicationSecret}
              type="password"
              autocomplete="off"
              required
            /></label
          ><label
            >Consumer Key<input
              bind:value={consumerKey}
              type="password"
              autocomplete="off"
              required
            /></label
          >{/if}
        <div class="modal-actions">
          <button type="button" onclick={() => (providerDialog = false)}>Cancel</button><button
            class="primary"
            disabled={savingProvider}>{savingProvider ? 'Encrypting…' : 'Encrypt & connect'}</button
          >
        </div>
      </form>
    </div>{/if}

  {#if selectedResource}<div
      class="drawer-backdrop"
      role="presentation"
      onclick={(event) => event.target === event.currentTarget && (selectedResource = null)}
    >
      <aside class="drawer">
        <div class="modal-head">
          <div>
            <p class="kicker">RESOURCE DETAIL</p>
            <h2>{selectedResource.name}</h2>
          </div>
          <button onclick={() => (selectedResource = null)}>×</button>
        </div>
        <div class="resource-identity">
          <span class="provider-logo large"
            >{selectedResource.resource_type === 'compute_instance' ? 'VM' : 'DN'}</span
          >
          <div>
            <span class="status {selectedResource.lifecycle}">{selectedResource.lifecycle}</span>
            <p>
              {selectedResource.resource_type.replaceAll('_', ' ')} · {selectedResource.region ??
                'global'}
            </p>
          </div>
        </div>
        {#if selectedResource.resource_type === 'compute_instance'}<div class="action-strip">
            {#each ['start', 'stop', 'reboot'] as action}<button
                onclick={() => selectedResource && lifecycle(selectedResource, action)}
                disabled={actionBusy === `${selectedResource.id}:${action}`}
                >{action === 'start' ? '▶' : action === 'stop' ? '■' : '↻'} {action}</button
              >{/each}
          </div>{/if}
        <section class="drawer-section">
          <div class="panel-head">
            <h3>Observed state</h3>
            <span class="beta">v{selectedResource.observed_state?.version ?? 0}</span>
          </div>
          <pre>{JSON.stringify(
              selectedResource.observed_state?.state ?? selectedResource.attributes,
              null,
              2,
            )}</pre>
        </section>
        <section class="drawer-section">
          <div class="panel-head">
            <h3>Configuration drift</h3>
            <span>{resourceDrifts.length}</span>
          </div>
          {#if detailLoading}<p class="muted">
              Loading state analysis…
            </p>{:else if resourceDrifts.length}{#each resourceDrifts as drift}<div
                class="drift-item"
              >
                <span class="status {drift.status}">{drift.status}</span>
                <div>
                  <strong>{Object.keys(drift.differences).length} managed differences</strong><small
                    >{relativeDate(drift.detected_at)}</small
                  >
                </div>
              </div>{/each}{:else}<div class="aligned-state">
              <span>✓</span>
              <div>
                <strong>No drift detected</strong><small
                  >Observed and desired fields are aligned.</small
                >
              </div>
            </div>{/if}
        </section>
        <section class="drawer-section">
          <div class="panel-head">
            <h3>Reconciliation</h3>
            <span>{reconciliations.length}</span>
          </div>
          {#each reconciliations as task}<div class="task-item">
              <div>
                <span class="status {task.status}">{task.status}</span><strong
                  >{task.policy.replaceAll('_', ' ')}</strong
                >
              </div>
              {#if task.status === 'pending' && task.policy === 'manual_approval'}<button
                  class="accent"
                  onclick={() => approve(task)}
                  disabled={actionBusy === task.id}>Approve</button
                >{/if}
            </div>{:else}<p class="muted">No reconciliation tasks.</p>{/each}
        </section>
      </aside>
    </div>{/if}
{/if}
