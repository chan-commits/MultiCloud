<script lang="ts">
  import { goto } from '$app/navigation';
  import OverviewView from '../components/OverviewView.svelte';
  import { getControlPlane } from '$lib/control-plane-context';
  import { relativeDate, shortId } from '$lib/format';

  const app = getControlPlane();
</script>

<OverviewView
  activeResources={app.activeResources}
  providers={app.providers}
  resources={app.resources}
  operations={app.operations}
  runningOperations={app.runningOperations}
  failedOperations={app.failedOperations}
  driftedResources={app.driftedResources}
  {relativeDate}
  {shortId}
  onOpenOperations={() => void goto('/operations')}
  onOpenProviders={() => void goto('/providers')}
/>
