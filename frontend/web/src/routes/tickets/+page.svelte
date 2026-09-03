<script lang="ts">
  import TicketsView from '../../components/TicketsView.svelte';
  import { getControlPlane } from '$lib/control-plane-context';
  import { relativeDate } from '$lib/format';
  const app = getControlPlane();
</script>

<TicketsView
  tickets={app.tickets}
  selected={app.selectedTicket}
  comments={app.ticketComments}
  actionBusy={app.actionBusy}
  onCreate={app.createTicket}
  onSelect={app.selectTicket}
  onStatus={app.updateTicket}
  onComment={app.addTicketComment}
  {relativeDate}
/>
