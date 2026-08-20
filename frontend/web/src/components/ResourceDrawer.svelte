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
  class="drawer-backdrop"
  role="presentation"
  onclick={(event) => event.target === event.currentTarget && onClose()}
>
  <aside class="drawer">
    <div class="modal-head">
      <div>
        <p class="kicker">RESOURCE DETAIL</p>
        <h2>{resource.name}</h2>
      </div>
      <button onclick={onClose}>×</button>
    </div>
    <div class="resource-identity">
      <span class="provider-logo large"
        >{resource.resource_type === 'compute_instance' ? 'VM' : 'DN'}</span
      >
      <div>
        <span class="status {resource.lifecycle}">{resource.lifecycle}</span>
        <p>{resource.resource_type.replaceAll('_', ' ')} · {resource.region ?? 'global'}</p>
      </div>
    </div>
    {#if resource.resource_type === 'compute_instance'}<div class="action-strip">
        {#each ['start', 'stop', 'reboot'] as action}<button
            onclick={() => onLifecycle(resource, action)}
            disabled={actionBusy === `${resource.id}:${action}`}
            >{action === 'start' ? '▶' : action === 'stop' ? '■' : '↻'} {action}</button
          >{/each}
      </div>{/if}
    <section class="drawer-section">
      <div class="panel-head">
        <h3>Observed state</h3>
        <span class="beta">v{resource.observed_state?.version ?? 0}</span>
      </div>
      <pre>{JSON.stringify(resource.observed_state?.state ?? resource.attributes, null, 2)}</pre>
    </section>
    <section class="drawer-section">
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
    <section class="drawer-section">
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
