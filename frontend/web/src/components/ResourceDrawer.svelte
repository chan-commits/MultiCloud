<script lang="ts">
  import type { Drift, Reconciliation, Resource } from '../lib/api';
  import { providerLogoClass, statusClass } from '../lib/ui';
  import { t } from '$lib/i18n.svelte';
  let {
    resource,
    drifts,
    reconciliations,
    loading,
    actionBusy,
    onClose,
    onLifecycle,
    onApprove,
    relativeDate,
  }: {
    resource: Resource;
    drifts: Drift[];
    reconciliations: Reconciliation[];
    loading: boolean;
    actionBusy: string;
    onClose: () => void;
    onLifecycle: (resource: Resource, action: string) => Promise<void>;
    onApprove: (task: Reconciliation) => Promise<void>;
    relativeDate: (value: string | null) => string;
  } = $props();
</script>

<div
  class="fixed inset-0 z-50 grid place-items-stretch justify-end bg-[#020508cc] backdrop-blur-[8px]"
  role="presentation"
  onclick={(event) => event.target === event.currentTarget && onClose()}
>
  <aside
    class="h-full w-[min(580px,100%)] overflow-auto border-l border-[#24404d] bg-[#080f17] p-[25px] shadow-[-20px_0_80px_#000]"
  >
    <div class="flex items-start justify-between border-b border-[#192a36] pb-[17px]">
      <div>
        <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
          {t('RESOURCE DETAIL')}
        </p>
        <h2 class="m-0 text-[30px] tracking-[-0.035em] text-[#f2f8ff]">{resource.name}</h2>
      </div>
      <button class="border-0 bg-transparent text-[22px] text-[#6c8292]" onclick={onClose}>×</button
      >
    </div>
    <div class="flex items-center gap-[13px] py-5">
      <span
        class={providerLogoClass(
          resource.resource_type === 'compute_instance' ? 'vultr' : 'cloudflare',
          true,
        )}>{resource.resource_type === 'compute_instance' ? 'VM' : 'DN'}</span
      >
      <div>
        <span class={statusClass(resource.lifecycle)}>{t(resource.lifecycle)}</span>
        <p class="m-[7px_0_0] text-[10px] capitalize text-[#667c8d]">
          {resource.resource_type.replaceAll('_', ' ')} · {resource.region ?? 'global'}
        </p>
      </div>
    </div>
    {#if resource.resource_type === 'compute_instance'}<div class="mb-5 flex gap-2">
        {#each ['start', 'stop', 'reboot'] as action}<button
            onclick={() => onLifecycle(resource, action)}
            disabled={actionBusy === `${resource.id}:${action}`}
            class="flex-1 border border-[#293c49] bg-[#0b141d] p-[9px] text-[10px] font-bold text-[#8ea4b3]"
            >{action === 'start' ? '▶' : action === 'stop' ? '■' : '↻'} {t(action)}</button
          >{/each}
      </div>{/if}
    <section class="mb-3 border border-[#192b37] bg-[#0a131c] p-4">
      <div class="mb-[18px] flex items-center justify-between">
        <h3 class="m-0 text-[14px] text-[#dce7ee]">{t('Observed state')}</h3>
        <span
          class="rounded-[3px] border border-[#2b4050] px-[6px] py-[3px] text-[8px] tracking-[0.12em] text-[#688397]"
          >v{resource.observed_state?.version ?? 0}</span
        >
      </div>
      <pre
        class="max-h-[260px] overflow-auto border border-[#14232d] bg-[#060b10] p-[13px] font-mono text-[10px] leading-[1.6] text-[#79b9bd]">{JSON.stringify(
          resource.observed_state?.state ?? resource.attributes,
          null,
          2,
        )}</pre>
    </section>
    <section class="mb-3 border border-[#192b37] bg-[#0a131c] p-4">
      <div class="mb-[18px] flex items-center justify-between">
        <h3 class="m-0 text-[14px] text-[#dce7ee]">{t('Configuration drift')}</h3>
        <span>{drifts.length}</span>
      </div>
      {#if loading}<p class="text-[9px] text-[#587080]">
          {t('Loading state analysis…')}
        </p>{:else if drifts.length}{#each drifts as drift}<div
            class="flex items-center gap-3 border-t border-[#172631] py-[10px]"
          >
            <span class={statusClass(drift.status)}>{t(drift.status)}</span>
            <div class="flex-1">
              <strong class="block text-[10px] text-[#b9c9d4]"
                >{t('{count} managed differences', {
                  count: Object.keys(drift.differences).length,
                })}</strong
              ><small class="block text-[9px] text-[#587080]"
                >{relativeDate(drift.detected_at)}</small
              >
            </div>
          </div>{/each}{:else}<div
          class="flex items-center gap-3 border-t border-[#172631] py-[10px]"
        >
          <span
            class="grid h-7 w-7 place-items-center rounded-full border border-[#246347] text-[#47dfa4]"
            >✓</span
          >
          <div>
            <strong class="block text-[10px] text-[#b9c9d4]">{t('No drift detected')}</strong><small
              class="block text-[9px] text-[#587080]"
              >{t('Observed and desired fields are aligned.')}</small
            >
          </div>
        </div>{/if}
    </section>
    <section class="mb-3 border border-[#192b37] bg-[#0a131c] p-4">
      <div class="mb-[18px] flex items-center justify-between">
        <h3 class="m-0 text-[14px] text-[#dce7ee]">{t('Reconciliation')}</h3>
        <span>{reconciliations.length}</span>
      </div>
      {#each reconciliations as task}<div
          class="flex items-center gap-3 border-t border-[#172631] py-[10px]"
        >
          <div class="flex-1">
            <span class={statusClass(task.status)}>{t(task.status)}</span><strong
              class="ml-2 text-[10px] capitalize text-[#95a9b7]"
              >{task.policy.replaceAll('_', ' ')}</strong
            >
          </div>
          {#if task.status === 'pending' && task.policy === 'manual_approval'}<button
              class="w-[90px] border border-[#17616a] bg-[#0c282d] p-[9px] text-[10px] font-bold text-[#60e5eb]"
              onclick={() => onApprove(task)}
              disabled={actionBusy === task.id}>{t('Approve')}</button
            >{/if}
        </div>{:else}<p class="text-[9px] text-[#587080]">{t('No reconciliation tasks.')}</p>{/each}
    </section>
  </aside>
</div>
