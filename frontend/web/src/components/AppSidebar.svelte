<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  type View = 'overview' | 'providers' | 'resources' | 'operations' | 'tickets' | 'audit';
  type NavigationItem = { id: View; label: string; caption: string; icon: string };
  let {
    navigation,
    view,
    mobileNav,
    organizationName,
    onNavigate,
    onLogout,
  }: {
    navigation: NavigationItem[];
    view: View;
    mobileNav: boolean;
    organizationName: string;
    onNavigate: (view: View) => void;
    onLogout: () => Promise<void>;
  } = $props();
</script>

<aside
  class={`fixed inset-y-0 left-0 z-20 flex w-[248px] flex-col border-r border-[#15222e] bg-[#070c12] px-[17px] py-[26px] transition-transform max-[760px]:-translate-x-full ${mobileNav ? 'max-[760px]:translate-x-0 max-[760px]:shadow-[20px_0_80px_#000]' : ''}`}
>
  <div class="border-b border-[#14212c] px-[10px] pb-[26px]">
    <div
      class="flex items-center gap-[11px] text-[18px] font-[750] text-[#eff8ff]
      "
    >
      <span
        class="grid h-[30px] w-[30px] rotate-45 place-items-center border border-brand-cyan text-[14px] text-brand-cyan"
        >M</span
      ><span>MultiCloud</span>
    </div>
  </div>
  <div
    class="my-5 flex items-center gap-[10px] rounded-[6px] border border-[#172735] bg-[#0b141e] p-[11px]"
  >
    <span
      class="grid h-[34px] w-[34px] min-w-0 place-items-center rounded-[5px] border border-[#236078] bg-gradient-to-br from-[#15394a] to-[#11232f] text-[10px] font-extrabold text-[#68e9ef]"
      >{organizationName.slice(0, 2).toUpperCase() || '--'}</span
    >
    <div class="min-w-0">
      <small
        class="mb-[3px] block overflow-hidden text-ellipsis whitespace-nowrap text-[8px] tracking-[0.13em] text-[#526b7c]"
        >{t('ACTIVE ORGANIZATION')}</small
      ><strong
        class="block overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-[#cedae4]"
        >{organizationName || t('Select tenant')}</strong
      >
    </div>
  </div>
  <nav class="flex flex-col gap-[5px]" aria-label={t('Primary navigation')}>
    {#each navigation as item}<button
        class={`flex items-center gap-3 rounded-[5px] border border-transparent bg-transparent p-[11px] text-left ${view === item.id ? 'border-[#17404c] bg-gradient-to-r from-[#0d2d36] to-[#0a151e] text-[#e3fbff] shadow-[inset_2px_0_var(--color-brand-cyan)]' : 'text-[#6d8294]'}`}
        onclick={() => onNavigate(item.id)}
      >
        <span class="w-6 text-center text-[17px] text-[#41cdd5]">{item.icon}</span><span
          ><strong class="block text-[12px]">{t(item.label)}</strong><small
            class="mt-[2px] block text-[9px] text-[#53697a]">{t(item.caption)}</small
          ></span
        >
      </button>{/each}
  </nav>
  <div class="mt-auto border-t border-[#14212c] px-[9px] pb-0 pt-[18px]">
    <div class="mb-[14px] flex items-center gap-[10px]">
      <i class="h-[7px] w-[7px] rounded-full bg-[#3ff1a7] shadow-[0_0_12px_#3ff1a7]"></i><span
        ><strong class="block text-[10px] text-[#b7c7d4]">{t('Control plane')}</strong><small
          class="block text-[9px] text-[#526979]">{t('All systems nominal')}</small
        ></span
      >
    </div>
    <button
      class="border-0 bg-transparent p-1 text-[11px] text-[#6f8799] hover:text-brand-cyan"
      onclick={onLogout}>{t('Sign out')}</button
    >
  </div>
</aside>
