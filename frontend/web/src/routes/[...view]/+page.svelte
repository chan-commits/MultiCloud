<script lang="ts">
  import App from '../../App.svelte';
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();
</script>

<App view={data.view} />
