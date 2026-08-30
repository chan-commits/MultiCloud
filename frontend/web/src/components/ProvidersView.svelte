<script lang="ts">
  import type { ProviderAccount } from '../lib/api';
  import { providerLogoClass, statusClass } from '../lib/ui';
  import { t } from '$lib/i18n.svelte';

  let {
    providers,
    actionBusy,
    relativeDate,
    onOpenDialog,
    onTestConnection,
    onSyncProvider,
  }: {
    providers: ProviderAccount[];
    actionBusy: string;
    relativeDate: (value: string | null) => string;
    onOpenDialog: () => void;
    onTestConnection: (provider: ProviderAccount) => Promise<void>;
    onSyncProvider: (provider: ProviderAccount) => Promise<void>;
  } = $props();
</script>

<section
  class="mb-7 flex items-end justify-between max-[760px]:flex-col max-[760px]:items-start max-[760px]:gap-5"
>
  <div>
    <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
      {t('ADAPTER REGISTRY')}
    </p>
    <h2>{t('Provider Fabric')}</h2>
    <p>{t('Encrypted credentials, capability discovery, and controlled synchronization.')}</p>
  </div>
  <button
    class="rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
    onclick={onOpenDialog}>＋ {t('Connect provider')}</button
  >
</section>
<section class="grid grid-cols-3 gap-[14px] max-[1100px]:grid-cols-2 max-[760px]:grid-cols-1">
  {#each providers as provider}
    <article class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-5">
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
        <span class={statusClass(provider.status)}>{t(provider.status)}</span>
      </div>
      <div class="grid grid-cols-2 gap-[18px] py-5">
        <div>
          <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">{t('CAPABILITIES')}</small>
          <p class="m-[5px_0_0] text-[10px] capitalize text-[#9db0bf]">
            {provider.capabilities.length
              ? provider.capabilities.join(' · ')
              : t('Awaiting discovery')}
          </p>
        </div>
        <div>
          <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">{t('LAST VERIFIED')}</small>
          <p class="m-[5px_0_0] text-[10px] capitalize text-[#9db0bf]">
            {relativeDate(provider.last_validated_at)}
          </p>
        </div>
        <div>
          <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">{t('CREDENTIAL')}</small>
          <p class="m-[5px_0_0] text-[10px] capitalize text-[#9db0bf]">
            {provider.credential_masked_identifier ?? t('Encrypted')}
          </p>
        </div>
        <div>
          <small class="text-[8px] tracking-[0.12em] text-[#4f6677]">{t('RISK')}</small>
          <p
            class={`m-[5px_0_0] text-[10px] capitalize ${provider.credential_risk_level === 'high' ? 'text-[#ffb35f]' : 'text-[#9db0bf]'}`}
          >
            {t(provider.credential_risk_level ?? 'restricted')}
          </p>
        </div>
      </div>
      {#if provider.last_error_code}<p class="bg-[#2a1710] p-2 text-[9px] text-[#ff9b65]">
          ⚠ {provider.last_error_code}
        </p>{/if}
      <div class="flex gap-2">
        <button
          class="flex-1 rounded-[4px] border border-[#293c49] bg-[#0b141d] p-[9px] text-[10px] font-bold text-[#8ea4b3]"
          onclick={() => onTestConnection(provider)}
          disabled={actionBusy === provider.id}>{t('Test connection')}</button
        >
        <button
          class="flex-1 rounded-[4px] border border-[#17616a] bg-[#0c282d] p-[9px] text-[10px] font-bold text-[#60e5eb]"
          onclick={() => onSyncProvider(provider)}
          disabled={provider.status !== 'active' || actionBusy === provider.id}
          >{t(actionBusy === provider.id ? 'Working…' : 'Sync inventory')}</button
        >
      </div>
    </article>
  {:else}
    <section
      class="col-span-full flex min-h-[400px] flex-col items-center justify-center border border-dashed border-[#233746] bg-[#091019] text-center text-[#617687]"
    >
      <div
        class="mb-[10px] grid h-[70px] w-[70px] place-items-center rounded-full border border-[#1f6771] text-[25px] text-[#37d7df] shadow-[0_0_30px_#19d4df15]"
      >
        ⌁
      </div>
      <h2 class="mb-1 text-[#c7d5df]">{t('No providers connected')}</h2>
      <p class="max-w-[430px] text-[12px]">
        {t('Connect Cloudflare, Vultr, or OVH to discover your first resources.')}
      </p>
      <button
        class="mt-[14px] rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
        onclick={onOpenDialog}>{t('Connect provider')}</button
      >
    </section>
  {/each}
</section>
