import { getContext } from 'svelte';
import type {
  AuditLog,
  Operation,
  ProviderAccount,
  Resource,
  Ticket,
  TicketComment,
} from '$lib/api';

export type ControlPlaneContext = {
  readonly providers: ProviderAccount[];
  readonly resources: Resource[];
  readonly operations: Operation[];
  readonly auditLogs: AuditLog[];
  readonly tickets: Ticket[];
  readonly ticketComments: TicketComment[];
  readonly selectedTicket: Ticket | null;
  readonly activeResources: Resource[];
  readonly driftedResources: Resource[];
  readonly runningOperations: Operation[];
  readonly failedOperations: Operation[];
  readonly actionBusy: string;
  readonly auditAction: string;
  readonly auditOutcome: string;
  readonly auditLoadingMore: boolean;
  readonly auditHasMore: boolean;
  openProviderDialog(): void;
  testConnection(provider: ProviderAccount): Promise<void>;
  syncProvider(provider: ProviderAccount): Promise<void>;
  openResource(resource: Resource): Promise<void>;
  cancel(operation: Operation): Promise<void>;
  setAuditAction(value: string): void;
  setAuditOutcome(value: string): void;
  applyAuditFilters(): Promise<void>;
  loadMoreAudit(): Promise<void>;
  exportAudit(): Promise<void>;
  createTicket(subject: string, description: string, priority: string): Promise<void>;
  selectTicket(ticket: Ticket): Promise<void>;
  updateTicket(ticket: Ticket, status: string): Promise<void>;
  addTicketComment(body: string): Promise<void>;
};

export const CONTROL_PLANE_CONTEXT = Symbol('control-plane');

export function getControlPlane(): ControlPlaneContext {
  return getContext<ControlPlaneContext>(CONTROL_PLANE_CONTEXT);
}
