<script lang="ts">
  import OperationsView from '../../components/OperationsView.svelte';
  import { getControlPlane } from '$lib/control-plane-context';
  import { relativeDate, shortId } from '$lib/format';

  const app = getControlPlane();
</script>

<OperationsView
  operations={app.operations}
  actionBusy={app.actionBusy}
  onCancel={app.cancel}
  {relativeDate}
  {shortId}
/>
