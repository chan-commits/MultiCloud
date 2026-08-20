<script lang="ts">
  import type { Resource } from '../lib/api';
  let {
    resources,
    onOpenResource,
    shortId,
  }: {
    resources: Resource[];
    onOpenResource: (resource: Resource) => Promise<void>;
    shortId: (value: string) => string;
  } = $props();
</script>

<section class="page-intro">
  <div>
    <p class="eyebrow">CANONICAL INVENTORY</p>
    <h2>Resource Matrix</h2>
    <p>Provider-neutral assets with normalized state and drift visibility.</p>
  </div>
  <div class="segmented">
    <button class="active">All {resources.length}</button><button
      >Compute {resources.filter((item) => item.resource_type === 'compute_instance')
        .length}</button
    ><button>DNS {resources.filter((item) => item.resource_type.startsWith('dns_')).length}</button>
  </div>
</section>
<section class="panel resource-panel">
  <div class="table-wrap">
    <table>
      <thead
        ><tr
          ><th>Resource</th><th>Type</th><th>Region</th><th>Lifecycle</th><th>Observed</th><th
          ></th></tr
        ></thead
      ><tbody>
        {#each resources as resource}<tr>
            <td
              ><div class="resource-name">
                <span>{resource.resource_type === 'compute_instance' ? '▣' : '◎'}</span>
                <div><strong>{resource.name}</strong><small>{shortId(resource.id)}</small></div>
              </div></td
            >
            <td>{resource.resource_type.replaceAll('_', ' ')}</td><td
              >{resource.region ?? 'global'}</td
            >
            <td><span class="status {resource.lifecycle}">{resource.lifecycle}</span></td><td
              >v{resource.observed_state?.version ?? 0}</td
            >
            <td
              ><button class="row-action" onclick={() => onOpenResource(resource)}>Inspect →</button
              ></td
            >
          </tr>{:else}<tr
            ><td colspan="6"
              ><div class="empty-row">
                <span>◇</span><strong>No resources discovered</strong><small
                  >Run inventory sync from Provider Fabric.</small
                >
              </div></td
            ></tr
          >{/each}
      </tbody>
    </table>
  </div>
</section>
