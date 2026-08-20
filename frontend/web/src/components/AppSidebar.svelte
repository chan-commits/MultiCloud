<script lang="ts">
  type View = 'overview' | 'providers' | 'resources' | 'operations' | 'audit';
  type NavigationItem = { id: View; label: string; caption: string; icon: string };
  let {
    navigation,
    view,
    mobileNav,
    organizationName,
    onNavigate,
    onLogout,
  }: {
    navigation: NavigationItem[];
    view: View;
    mobileNav: boolean;
    organizationName: string;
    onNavigate: (view: View) => void;
    onLogout: () => Promise<void>;
  } = $props();
</script>

<aside class:open={mobileNav}>
  <div class="brand sidebar-brand"><span class="brand-glyph">M</span><span>MultiCloud</span></div>
  <div class="tenant-chip">
    <span class="tenant-avatar">{organizationName.slice(0, 2).toUpperCase() || '--'}</span>
    <div>
      <small>ACTIVE ORGANIZATION</small><strong>{organizationName || 'Select tenant'}</strong>
    </div>
  </div>
  <nav aria-label="Primary navigation">
    {#each navigation as item}<button
        class:active={view === item.id}
        onclick={() => onNavigate(item.id)}
      >
        <span class="nav-icon">{item.icon}</span><span
          ><strong>{item.label}</strong><small>{item.caption}</small></span
        >
      </button>{/each}
  </nav>
  <div class="sidebar-foot">
    <div class="system-health">
      <i></i><span><strong>Control plane</strong><small>All systems nominal</small></span>
    </div>
    <button class="text-button" onclick={onLogout}>Sign out</button>
  </div>
</aside>
