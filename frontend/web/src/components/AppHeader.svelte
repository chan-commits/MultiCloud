<script lang="ts">
  import type { Organization } from '../lib/api';
  type View = 'overview' | 'providers' | 'resources' | 'operations' | 'audit';
  let {
    view,
    navigation,
    organizations,
    organizationId,
    isPlatformAdmin,
    registrationEnabled,
    registrationBusy,
    loading,
    onMenu,
    onOrganizationChange,
    onRefresh,
    onToggleRegistration,
  }: {
    view: View;
    navigation: { id: View; label: string; caption: string; icon: string }[];
    organizations: Organization[];
    organizationId: string;
    isPlatformAdmin: boolean;
    registrationEnabled: boolean;
    registrationBusy: boolean;
    loading: boolean;
    onMenu: () => void;
    onOrganizationChange: (event: Event) => void;
    onRefresh: () => Promise<void>;
    onToggleRegistration: () => Promise<void>;
  } = $props();
</script>

<header class="topbar">
  <button class="menu-button" aria-label="Toggle navigation" onclick={onMenu}>☰</button>
  <div>
    <p class="breadcrumb">CONTROL PLANE / <span>{view.toUpperCase()}</span></p>
    <h1>{navigation.find((item) => item.id === view)?.label}</h1>
  </div>
  <div class="top-actions">
    {#if isPlatformAdmin}<button
        class="registration-toggle"
        class:enabled={registrationEnabled}
        onclick={onToggleRegistration}
        disabled={registrationBusy || !organizationId}
        title="Platform-wide public registration"
        ><i></i>{registrationEnabled ? 'Registration on' : 'Registration off'}</button
      >{/if}
    <select aria-label="Organization" value={organizationId} onchange={onOrganizationChange}
      >{#each organizations as organization}<option value={organization.id}
          >{organization.name}</option
        >{/each}</select
    >
    <button class="icon-button" aria-label="Refresh data" onclick={onRefresh} disabled={loading}
      >↻</button
    >
    <span class="operator-avatar">OP</span>
  </div>
</header>
