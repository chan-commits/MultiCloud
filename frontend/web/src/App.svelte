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
  import { providerLogoClass, statusClass } from './lib/ui';

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
      {#if error}<div
          class="mx-8 mt-[18px] flex items-center gap-[10px] rounded-[5px] border border-[#60202e] bg-[#2b1118] px-[14px] py-[10px] text-[12px] text-[#ff788e] max-[760px]:mx-[14px] max-[760px]:mt-3"
        >
          <span>!</span>
          <p class="m-0 flex-1">{error}</p>
          <button class="border-0 bg-transparent" onclick={() => (error = '')}>×</button>
        </div>{/if}{#if notice}<div
          class="mx-8 mt-[18px] flex items-center gap-[10px] rounded-[5px] border border-[#205846] bg-[#0d211a] px-[14px] py-[10px] text-[12px] text-[#54d7a1] max-[760px]:mx-[14px] max-[760px]:mt-3"
        >
          <span>✓</span>
          <p class="m-0 flex-1">{notice}</p>
          <button class="border-0 bg-transparent" onclick={() => (notice = '')}>×</button>
        </div>{/if}
      <div
        class:loading
        class="mx-auto max-w-[1600px] px-8 pb-[60px] pt-[30px] transition-opacity max-[760px]:px-[14px] max-[760px]:pb-[50px] max-[760px]:pt-5"
      >
        {#if !organizationId}<OrganizationOnboarding
            creating={creatingOrganization}
            onCreate={createOrganization}
          />
        {:else if view === 'overview'}
          <section
            class="relative mb-[14px] flex min-h-[260px] items-center justify-between overflow-hidden border border-[#1b4b58] bg-[radial-gradient(circle_at_75%_50%,#0d3640,#0a151e_58%)] p-10 max-[760px]:min-h-[250px] max-[760px]:p-[30px]"
          >
            <div>
              <p
                class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-[var(--cyan)]"
              >
                LIVE INFRASTRUCTURE POSTURE
              </p>
              <h2 class="m-0 text-[34px] tracking-[-0.045em] text-[#edf6fb]">
                Your cloud estate,<br /><span class="text-[#65e8ed]">resolved in real time.</span>
              </h2>
              <p class="max-w-[500px] text-[12px] leading-[1.7] text-[#78909f]">
                Unified visibility across every connected provider and canonical resource.
              </p>
            </div>
            <div
              class="relative mr-[8%] grid h-[128px] w-[128px] place-items-center rounded-full border border-[#2ebac5] bg-[#0b222a] shadow-[0_0_50px_#19d4df33] max-[760px]:hidden"
            >
              <div
                class="absolute inset-[10px] rounded-full border border-dashed border-[#2ebac5] opacity-60"
              ></div>
              <strong class="text-[31px] text-[#d9fbff]">{activeResources.length}</strong><small
                class="absolute bottom-[28px] text-[8px] tracking-[0.15em] text-[#69cdd3]"
                >ACTIVE</small
              >
            </div>
          </section>
          <section
            class="mb-[14px] grid grid-cols-4 gap-[14px] max-[1100px]:grid-cols-2 max-[760px]:grid-cols-1"
          >
            <article
              class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]"
            >
              <div
                class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
              >
                <span>CONNECTED PROVIDERS</span><i class="text-[18px] not-italic text-[#24dbe3]"
                  >⌁</i
                >
              </div>
              <strong class="mt-4 block text-[30px] text-[#dcecf4]"
                >{providers.filter((item) => item.status === 'active').length}<small>
                  / {providers.length}</small
                ></strong
              >
              <p class="text-[10px] text-[#718798]">
                <span class="text-[#3fe4a2]">●</span> Capability registry online
              </p>
            </article>
            <article
              class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]"
            >
              <div
                class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
              >
                <span>MANAGED RESOURCES</span><i class="text-[18px] not-italic text-[#a48bff]">◇</i>
              </div>
              <strong class="mt-4 block text-[30px] text-[#dcecf4]">{resources.length}</strong>
              <p class="text-[10px] text-[#718798]">{activeResources.length} currently active</p>
            </article>
            <article
              class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]"
            >
              <div
                class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
              >
                <span>ACTIVE OPERATIONS</span><i class="text-[18px] not-italic text-[#ffc064]">↯</i>
              </div>
              <strong class="mt-4 block text-[30px] text-[#dcecf4]"
                >{runningOperations.length}</strong
              >
              <p class="text-[10px] text-[#718798]">
                {failedOperations.length} failures in recent history
              </p>
            </article>
            <article
              class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]"
            >
              <div
                class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
              >
                <span>CONFIGURATION DRIFT</span><i class="text-[18px] not-italic text-[#ff718b]"
                  >∆</i
                >
              </div>
              <strong class="mt-4 block text-[30px] text-[#dcecf4]"
                >{driftedResources.length}</strong
              >
              <p class="text-[10px] text-[#718798]">
                {driftedResources.length ? 'Review required' : 'Desired state aligned'}
              </p>
            </article>
          </section>
          <section
            class="grid grid-cols-2 gap-[14px] max-[1100px]:grid-cols-2 max-[760px]:grid-cols-1"
          >
            <article class="col-span-2 border border-[#1b2e3b] bg-panel p-5 max-[760px]:col-span-1">
              <div
                class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3"
              >
                <div>
                  <p
                    class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-[var(--cyan)]"
                  >
                    OPERATION TELEMETRY
                  </p>
                  <h3 class="m-0 text-[14px] text-[#c5d6df]">Execution stream</h3>
                </div>
                <button
                  class="border-0 bg-transparent p-1 text-[11px] text-[#6f8799] hover:text-[var(--cyan)]"
                  onclick={() => (view = 'operations')}>View all →</button
                >
              </div>
              <div class="relative h-[195px] overflow-hidden">
                <div
                  class="absolute inset-[10px_0_18px] bg-[linear-gradient(#29404d35_1px,transparent_1px),linear-gradient(90deg,#29404d25_1px,transparent_1px)] bg-[length:100%_33%,12.5%_100%]"
                ></div>
                <svg
                  viewBox="0 0 800 180"
                  preserveAspectRatio="none"
                  class="absolute inset-[10px_0_18px] h-[calc(100%-28px)] w-full overflow-visible"
                  ><path
                    class="fill-none stroke-[#26d5df] [filter:drop-shadow(0_0_6px_#26d5df77)] [stroke-width:2] [vector-effect:non-scaling-stroke]"
                    d="M0 145 C70 130 90 135 150 105 S250 135 315 80 S420 115 485 55 S590 90 650 38 S750 55 800 20"
                  /></svg
                ><span class="absolute bottom-0 right-1 text-[8px] text-[#425968]"
                  >Time-series telemetry connects in Observability phase</span
                >
              </div>
            </article>
            <article class="border border-[#1b2e3b] bg-panel p-5">
              <div
                class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3"
              >
                <div>
                  <p
                    class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-[var(--cyan)]"
                  >
                    PROVIDER FABRIC
                  </p>
                  <h3 class="m-0 text-[14px] text-[#c5d6df]">Connection status</h3>
                </div>
              </div>
              <div class="flex flex-col gap-2">
                {#each providers.slice(0, 4) as provider}<button
                    class="flex items-center gap-[10px] border border-[#172633] bg-[#0a1119] p-[9px] text-left text-[#b8c9d5]"
                    onclick={() => (view = 'providers')}
                    ><span class={providerLogoClass(provider.provider_kind)}
                      >{provider.provider_kind.slice(0, 2).toUpperCase()}</span
                    ><span class="flex-1"
                      ><strong class="block text-[11px]">{provider.name}</strong><small
                        class="block text-[9px] uppercase text-[#536c7e]"
                        >{provider.provider_kind}</small
                      ></span
                    ><i
                      class={`h-[6px] w-[6px] rounded-full ${provider.status === 'active' ? 'bg-[#3de6a1] shadow-[0_0_9px_#3de6a1]' : 'bg-[#77404a]'}`}
                    ></i></button
                  >{:else}<div class="p-[30px] text-center text-[11px] text-[#526a7a]">
                    Connect a provider to activate the fabric.
                  </div>{/each}
              </div>
            </article>
            <article class="col-span-2 border border-[#1b2e3b] bg-panel p-5 max-[760px]:col-span-1">
              <div
                class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3"
              >
                <div>
                  <p
                    class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-[var(--cyan)]"
                  >
                    RECENT ACTIVITY
                  </p>
                  <h3 class="m-0 text-[14px] text-[#c5d6df]">Operation ledger</h3>
                </div>
              </div>
              <div
                class="overflow-auto [&_table]:w-full [&_table]:min-w-[600px] [&_table]:border-collapse [&_th]:border-b [&_th]:border-[#1b2a36] [&_th]:p-[9px] [&_th]:text-left [&_th]:text-[8px] [&_th]:tracking-[0.14em] [&_th]:text-[#52697b] [&_td]:border-b [&_td]:border-[#14212b] [&_td]:p-[12px_9px] [&_td]:text-[10px] [&_td]:text-[#8fa3b3] [&_td_strong]:block [&_td_strong]:text-[11px] [&_td_strong]:text-[#bfced8] [&_td_small]:mt-[3px] [&_td_small]:block [&_td_small]:text-[8px] [&_td_small]:text-[#4e6677]"
              >
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
                          ><span class={statusClass(operation.status)}>{operation.status}</span></td
                        ><td
                          ><div class="relative h-[3px] w-[100px] bg-[#1b2934]">
                            <i
                              class="block h-full bg-[#1bd4de] shadow-[0_0_7px_#1bd4de]"
                              style={`width:${operation.progress}%`}
                            ></i>
                          </div></td
                        ><td>{relativeDate(operation.created_at)}</td></tr
                      >{:else}<tr
                        ><td colspan="5" class="p-[30px] text-center text-[11px] text-[#526a7a]"
                          >No operations recorded yet.</td
                        ></tr
                      >{/each}</tbody
                  >
                </table>
              </div>
            </article>
            <article class="border border-[#1b2e3b] bg-panel p-5">
              <div
                class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3"
              >
                <div>
                  <p
                    class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-[var(--cyan)]"
                  >
                    COST SIGNAL
                  </p>
                  <h3 class="m-0 text-[14px] text-[#c5d6df]">Cloud spend</h3>
                </div>
                <span
                  class="rounded-[3px] border border-[#2b4050] px-[6px] py-[3px] text-[8px] tracking-[0.12em] text-[#688397]"
                  >PHASE 9</span
                >
              </div>
              <div>
                <span class="text-[38px] text-[#37505e]">—</span>
                <p class="text-[11px] leading-[1.5] text-[#5c7181]">
                  Billing telemetry is reserved for the Billing bounded context.
                </p>
                <div class="flex h-[75px] items-end gap-[7px] opacity-30">
                  <i class="h-[30%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"></i><i
                    class="h-[55%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"
                  ></i><i class="h-[40%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"></i><i
                    class="h-[75%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"
                  ></i><i class="h-[62%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"></i><i
                    class="h-[90%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"
                  ></i>
                </div>
              </div>
            </article>
          </section>
        {:else if view === 'providers'}
          <section
            class="mb-7 flex items-end justify-between max-[760px]:flex-col max-[760px]:items-start max-[760px]:gap-5"
          >
            <div>
              <p
                class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-[var(--cyan)]"
              >
                ADAPTER REGISTRY
              </p>
              <h2>Provider Fabric</h2>
              <p>Encrypted credentials, capability discovery, and controlled synchronization.</p>
            </div>
            <button
              class="rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
              onclick={() => (providerDialog = true)}>＋ Connect provider</button
            >
          </section>
          <section
            class="grid grid-cols-3 gap-[14px] max-[1100px]:grid-cols-2 max-[760px]:grid-cols-1"
          >
            {#each providers as provider}<article
                class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-5"
              >
                <div class="flex items-center gap-3 border-b border-[#182733] pb-[18px]">
                  <span class={providerLogoClass(provider.provider_kind, true)}
                    >{provider.provider_kind.slice(0, 2).toUpperCase()}</span
                  >
                  <div class="flex-1">
                    <p class="m-0 mb-1 text-[8px] tracking-[0.14em] text-[#526b7d]">
                      {provider.provider_kind.toUpperCase()}
                    </p>
                    <h3 class="m-0 text-[15px] text-[#dce8ef]">{provider.name}</h3>
                  </div>
                  <span class={statusClass(provider.status)}>{provider.status}</span>
                </div>
                <div class="grid grid-cols-2 gap-[18px] py-5">
                  <div>
                    <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">CAPABILITIES</small>
                    <p class="m-[5px_0_0] text-[10px] capitalize text-[#9db0bf]">
                      {provider.capabilities.length
                        ? provider.capabilities.join(' · ')
                        : 'Awaiting discovery'}
                    </p>
                  </div>
                  <div>
                    <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">LAST VERIFIED</small>
                    <p class="m-[5px_0_0] text-[10px] capitalize text-[#9db0bf]">
                      {relativeDate(provider.last_validated_at)}
                    </p>
                  </div>
                  <div>
                    <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">CREDENTIAL</small>
                    <p class="m-[5px_0_0] text-[10px] capitalize text-[#9db0bf]">
                      {provider.credential_masked_identifier ?? 'Encrypted'}
                    </p>
                  </div>
                  <div>
                    <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">RISK</small>
                    <p
                      class={`m-[5px_0_0] text-[10px] capitalize ${provider.credential_risk_level === 'high' ? 'text-[#ffb35f]' : 'text-[#9db0bf]'}`}
                    >
                      {provider.credential_risk_level ?? 'restricted'}
                    </p>
                  </div>
                </div>
                {#if provider.last_error_code}<p class="bg-[#2a1710] p-2 text-[9px] text-[#ff9b65]">
                    ⚠ {provider.last_error_code}
                  </p>{/if}
                <div class="flex gap-2">
                  <button
                    class="flex-1 rounded-[4px] border border-[#293c49] bg-[#0b141d] p-[9px] text-[10px] font-bold text-[#8ea4b3]"
                    onclick={() => testConnection(provider)}
                    disabled={actionBusy === provider.id}>Test connection</button
                  ><button
                    class="flex-1 rounded-[4px] border border-[#17616a] bg-[#0c282d] p-[9px] text-[10px] font-bold text-[#60e5eb]"
                    onclick={() => syncProvider(provider)}
                    disabled={provider.status !== 'active' || actionBusy === provider.id}
                    >{actionBusy === provider.id ? 'Working…' : 'Sync inventory'}</button
                  >
                </div>
              </article>{:else}<section
                class="col-span-full flex min-h-[400px] flex-col items-center justify-center border border-dashed border-[#233746] bg-[#091019] text-center text-[#617687]"
              >
                <div
                  class="mb-[10px] grid h-[70px] w-[70px] place-items-center rounded-full border border-[#1f6771] text-[25px] text-[#37d7df] shadow-[0_0_30px_#19d4df15]"
                >
                  ⌁
                </div>
                <h2 class="mb-1 text-[#c7d5df]">No providers connected</h2>
                <p class="max-w-[430px] text-[12px]">
                  Connect Cloudflare, Vultr, or OVH to discover your first resources.
                </p>
                <button
                  class="mt-[14px] rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
                  onclick={() => (providerDialog = true)}>Connect provider</button
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
