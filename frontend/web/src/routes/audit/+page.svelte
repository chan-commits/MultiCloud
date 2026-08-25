<script lang="ts">
  import AuditView from '../../components/AuditView.svelte';
  import { getControlPlane } from '$lib/control-plane-context';
  import { relativeDate, shortId } from '$lib/format';

  const app = getControlPlane();
</script>

<AuditView
  auditLogs={app.auditLogs}
  auditAction={app.auditAction}
  auditOutcome={app.auditOutcome}
  auditLoadingMore={app.auditLoadingMore}
  auditHasMore={app.auditHasMore}
  actionBusy={app.actionBusy}
  onActionChange={app.setAuditAction}
  onOutcomeChange={app.setAuditOutcome}
  onApplyFilters={app.applyAuditFilters}
  onLoadMore={app.loadMoreAudit}
  onExport={app.exportAudit}
  {relativeDate}
  {shortId}
/>
