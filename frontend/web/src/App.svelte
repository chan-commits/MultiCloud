<script lang="ts">
  import { onMount } from 'svelte';
  import AuthScreen from './components/AuthScreen.svelte';
  import AppHeader from './components/AppHeader.svelte';
  import AppSidebar from './components/AppSidebar.svelte';
  import OrganizationOnboarding from './components/OrganizationOnboarding.svelte';
  import AuditView from './components/AuditView.svelte';
  import OperationsView from './components/OperationsView.svelte';
  import ProviderDialog from './components/ProviderDialog.svelte';
  import ResourceDrawer from './components/ResourceDrawer.svelte';
  import ResourcesView from './components/ResourcesView.svelte';
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
  let actionBusy = $state('');
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

  async function changeOrganization(event?: Event) {
    if (event?.currentTarget instanceof HTMLSelectElement)
      organizationId = event.currentTarget.value;
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

  async function createProvider(payload: Record<string, unknown>) {
    if (!client) return;
    savingProvider = true;
    error = '';
    try {
      await client.createProvider(payload);
      providerDialog = false;
      notice = 'Provider account encrypted and ready for validation.';
      await refreshAll();
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      savingProvider = false;
    }
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
    <AppSidebar
      {navigation}
      {view}
      {mobileNav}
      organizationName={activeOrganization?.name ?? ''}
      onNavigate={(nextView) => {
        view = nextView;
        mobileNav = false;
      }}
      onLogout={logout}
    />
    <main class="workspace">
      <AppHeader
        {view}
        {navigation}
        {organizations}
        {organizationId}
        {isPlatformAdmin}
        {registrationEnabled}
        {registrationBusy}
        {loading}
        onMenu={() => (mobileNav = !mobileNav)}
        onOrganizationChange={changeOrganization}
        onRefresh={refreshAll}
        onToggleRegistration={toggleRegistration}
      />
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
          <ResourcesView {resources} onOpenResource={openResource} {shortId} />
        {:else if view === 'operations'}
          <OperationsView {operations} {actionBusy} onCancel={cancel} {relativeDate} {shortId} />
        {:else}
          <AuditView
            {auditLogs}
            {auditAction}
            {auditOutcome}
            {auditLoadingMore}
            {auditHasMore}
            {actionBusy}
            onActionChange={(value) => (auditAction = value)}
            onOutcomeChange={(value) => (auditOutcome = value)}
            onApplyFilters={applyAuditFilters}
            onLoadMore={loadMoreAudit}
            onExport={exportAudit}
            {relativeDate}
            {shortId}
          />
        {/if}
      </div>
    </main>
  </div>

  {#if providerDialog}<ProviderDialog
      saving={savingProvider}
      onClose={() => (providerDialog = false)}
      onCreate={createProvider}
    />{/if}

  {#if selectedResource}<ResourceDrawer
      resource={selectedResource}
      drifts={resourceDrifts}
      {reconciliations}
      loading={detailLoading}
      {actionBusy}
      onClose={() => (selectedResource = null)}
      onLifecycle={lifecycle}
      onApprove={approve}
      {relativeDate}
    />{/if}
{/if}
