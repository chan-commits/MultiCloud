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

<header
  class="sticky top-0 z-10 flex h-[82px] items-center justify-between border-b border-[#17232e] bg-[#070c12db] px-8 backdrop-blur-[14px] max-[760px]:h-[72px] max-[760px]:px-[15px]"
>
  <button
    class="hidden h-9 w-9 rounded-[5px] border border-[#1b2b38] bg-[#0b131c] text-[#8ba0b0] max-[760px]:block"
    aria-label="Toggle navigation"
    onclick={onMenu}>☰</button
  >
  <div>
    <p class="m-0 text-[8px] font-extrabold tracking-[0.18em] text-[#536777]">
      CONTROL PLANE / <span class="text-[#19bac5]">{view.toUpperCase()}</span>
    </p>
    <h1 class="m-[4px_0_0] text-[20px] tracking-[-0.025em] text-[#e8f2f9]">
      {navigation.find((item) => item.id === view)?.label}
    </h1>
  </div>
  <div class="flex items-center gap-[10px]">
    {#if isPlatformAdmin}<button
        class={`flex items-center gap-[7px] rounded-[5px] border px-[10px] py-[9px] text-[9px] ${registrationEnabled ? 'border-[#205846] bg-[#0d211a] text-[#54d7a1]' : 'border-[#49313a] bg-[#1b1117] text-[#aa7c88]'}`}
        onclick={onToggleRegistration}
        disabled={registrationBusy || !organizationId}
        title="Platform-wide public registration"
        ><i
          class={`h-[6px] w-[6px] rounded-full ${registrationEnabled ? 'bg-[#3de6a1] shadow-[0_0_8px_#3de6a1]' : 'bg-[#ad5367]'}`}
        ></i>{registrationEnabled ? 'Registration on' : 'Registration off'}</button
      >{/if}
    <select
      class="max-w-[190px] rounded-[6px] border border-[#203140] bg-[#09111a] px-[11px] py-[9px] pr-8 text-[11px] text-[#eaf6ff] outline-none"
      aria-label="Organization"
      value={organizationId}
      onchange={onOrganizationChange}
      >{#each organizations as organization}<option value={organization.id}
          >{organization.name}</option
        >{/each}</select
    >
    <button
      class="h-9 w-9 rounded-[5px] border border-[#1b2b38] bg-[#0b131c] text-[#8ba0b0]"
      aria-label="Refresh data"
      onclick={onRefresh}
      disabled={loading}>↻</button
    >
    <span
      class="grid h-[34px] w-[34px] place-items-center rounded-[5px] border border-[#236078] bg-gradient-to-br from-[#15394a] to-[#11232f] text-[10px] font-extrabold text-[#68e9ef] max-[760px]:hidden"
      >OP</span
    >
  </div>
</header>
