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
  import { messageOf, relativeDate, shortId } from './lib/format';
  import { navigation, type View } from './lib/navigation';
  import {
    approveReconciliation,
    cancelOperation,
    createOrganization as createOrganizationAction,
    exportAudit as exportAuditAction,
    loadResourceDetails,
    providerSync,
    providerTest,
    queryAudit,
    resourceOperation,
    resourceProvider,
  } from './lib/app-actions';
  import {
    activeResourcesOf,
    driftedResourcesOf,
    failedOperationsOf,
    runningOperationsOf,
  } from './lib/app-state.svelte';
  import {
    clearSession,
    persistOrganization,
    persistSession,
    readPreferredOrganization,
    readSession,
  } from './lib/session';

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
  let activeResources = $derived(activeResourcesOf(resources));
  let driftedResources = $derived(driftedResourcesOf(resources));
  let runningOperations = $derived(runningOperationsOf(operations));
  let failedOperations = $derived(failedOperationsOf(operations));

  onMount(async () => {
    try {
      const settings = await registrationSettings();
      registrationEnabled = settings.registration_enabled;
      platformInitialized = settings.initialized;
    } catch {
      loginError = 'Could not load platform registration status.';
    }
    const session = readSession();
    if (!session) return;
    try {
      token = session.token;
      isPlatformAdmin = session.isPlatformAdmin ?? false;
      await initialize();
    } catch {
      clearSession();
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
      persistSession({
        token,
        expiresAt: session.expires_at,
        isPlatformAdmin: session.is_platform_admin,
      });
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
      persistSession({
        token,
        expiresAt: session.expires_at,
        isPlatformAdmin: session.is_platform_admin,
      });
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
      const preferred = readPreferredOrganization();
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
    persistOrganization(organizationId);
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
        queryAudit(client, auditFilters()),
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
      const organization = await createOrganizationAction(
        client,
        organizationName,
        organizationSlug,
      );
      organizations = [...organizations, organization];
      organizationId = organization.id;
      client.setOrganization(organization.id);
      persistOrganization(organization.id);
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
      auditLogs = await queryAudit(client, auditFilters());
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
      const rows = await queryAudit(client, auditFilters(auditLogs.at(-1)));
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
    clearSession();
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
      const result = await providerTest(client, provider);
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
      await providerSync(client, provider);
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
      [resourceDrifts, reconciliations] = await loadResourceDetails(client, resource.id);
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      detailLoading = false;
    }
  }
  async function lifecycle(resource: Resource, action: string) {
    if (!client) return;
    const provider = resourceProvider(providers, resource),
      externalId = resource.external_id ?? '';
    if (!provider || !externalId) {
      error = 'This resource does not expose an operable provider mapping yet.';
      return;
    }
    actionBusy = `${resource.id}:${action}`;
    try {
      await resourceOperation(client, provider, resource, action);
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
      await approveReconciliation(client, selectedResource.id, task);
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
      await cancelOperation(client, operation);
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
      await exportAuditAction(client, auditFilters());
      notice = 'Sanitized audit export generated.';
    } catch (cause) {
      error = messageOf(cause);
    } finally {
      actionBusy = '';
    }
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
