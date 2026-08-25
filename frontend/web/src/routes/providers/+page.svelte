<script lang="ts">
  import ProvidersView from '../../components/ProvidersView.svelte';
  import { getControlPlane } from '$lib/control-plane-context';
  import { relativeDate } from '$lib/format';

  const app = getControlPlane();
</script>

<ProvidersView
  providers={app.providers}
  actionBusy={app.actionBusy}
  {relativeDate}
  onOpenDialog={app.openProviderDialog}
  onTestConnection={app.testConnection}
  onSyncProvider={app.syncProvider}
/>
