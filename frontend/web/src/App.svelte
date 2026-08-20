<script lang="ts">
  import { onMount } from 'svelte';
  import AuthScreen from './components/AuthScreen.svelte';
  import AppHeader from './components/AppHeader.svelte';
  import AppSidebar from './components/AppSidebar.svelte';
  import AppAlerts from './components/AppAlerts.svelte';
  import OverviewView from './components/OverviewView.svelte';
  import ProvidersView from './components/ProvidersView.svelte';
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
  <div class="grid min-h-screen grid-cols-[248px_minmax(0,1fr)] max-[760px]:block">
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
    <main class="min-w-0 [grid-column:2] max-[760px]:[grid-column:auto]">
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
      <AppAlerts
        {error}
        {notice}
        onDismissError={() => (error = '')}
        onDismissNotice={() => (notice = '')}
      />
      <div
        class:loading
        class="mx-auto max-w-[1600px] px-8 pb-[60px] pt-[30px] transition-opacity max-[760px]:px-[14px] max-[760px]:pb-[50px] max-[760px]:pt-5"
      >
        {#if !organizationId}<OrganizationOnboarding
            creating={creatingOrganization}
            onCreate={createOrganization}
          />
        {:else if view === 'overview'}
          <OverviewView
            {activeResources}
            {providers}
            {resources}
            {operations}
            {runningOperations}
            {failedOperations}
            {driftedResources}
            {relativeDate}
            {shortId}
            onOpenOperations={() => (view = 'operations')}
            onOpenProviders={() => (view = 'providers')}
          />
        {:else if view === 'providers'}
          <ProvidersView
            {providers}
            {actionBusy}
            {relativeDate}
            onOpenDialog={() => (providerDialog = true)}
            onTestConnection={testConnection}
            onSyncProvider={syncProvider}
          />
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
