<script lang="ts">
  import type { Drift, Reconciliation, Resource } from '../lib/api';
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
        <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-[var(--cyan)]">
          RESOURCE DETAIL
        </p>
        <h2 class="m-0 text-[30px] tracking-[-0.035em] text-[#f2f8ff]">{resource.name}</h2>
      </div>
      <button class="border-0 bg-transparent text-[22px] text-[#6c8292]" onclick={onClose}>×</button
      >
    </div>
    <div class="flex items-center gap-[13px] py-5">
      <span class="provider-logo large"
        >{resource.resource_type === 'compute_instance' ? 'VM' : 'DN'}</span
      >
      <div>
        <span class="status {resource.lifecycle}">{resource.lifecycle}</span>
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
            >{action === 'start' ? '▶' : action === 'stop' ? '■' : '↻'} {action}</button
          >{/each}
      </div>{/if}
    <section class="mb-3 border border-[#192b37] bg-[#0a131c] p-4">
      <div class="panel-head">
        <h3>Observed state</h3>
        <span class="beta">v{resource.observed_state?.version ?? 0}</span>
      </div>
      <pre>{JSON.stringify(resource.observed_state?.state ?? resource.attributes, null, 2)}</pre>
    </section>
    <section class="mb-3 border border-[#192b37] bg-[#0a131c] p-4">
      <div class="panel-head">
        <h3>Configuration drift</h3>
        <span>{drifts.length}</span>
      </div>
      {#if loading}<p class="muted">
          Loading state analysis…
        </p>{:else if drifts.length}{#each drifts as drift}<div class="drift-item">
            <span class="status {drift.status}">{drift.status}</span>
            <div>
              <strong>{Object.keys(drift.differences).length} managed differences</strong><small
                >{relativeDate(drift.detected_at)}</small
              >
            </div>
          </div>{/each}{:else}<div class="aligned-state">
          <span>✓</span>
          <div>
            <strong>No drift detected</strong><small>Observed and desired fields are aligned.</small
            >
          </div>
        </div>{/if}
    </section>
    <section class="mb-3 border border-[#192b37] bg-[#0a131c] p-4">
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
              onclick={() => onApprove(task)}
              disabled={actionBusy === task.id}>Approve</button
            >{/if}
        </div>{:else}<p class="muted">No reconciliation tasks.</p>{/each}
    </section>
  </aside>
</div>
