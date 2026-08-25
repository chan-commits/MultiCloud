<script lang="ts">
  import ResourcesView from '../../components/ResourcesView.svelte';
  import { getControlPlane } from '$lib/control-plane-context';
  import { shortId } from '$lib/format';

  const app = getControlPlane();
</script>

<ResourcesView resources={app.resources} onOpenResource={app.openResource} {shortId} />
