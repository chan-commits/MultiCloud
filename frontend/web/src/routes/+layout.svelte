<script lang="ts">
  import { page } from '$app/state';
  import App from '../App.svelte';
  import { viewFromRoute } from '$lib/navigation';
  import '../styles/index.css';

  let { children } = $props();
  let view = $derived(viewFromRoute(page.url.pathname.split('/').filter(Boolean)[0]));
</script>

<App {view}>{children}</App>
