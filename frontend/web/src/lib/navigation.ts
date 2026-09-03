export type View = 'overview' | 'providers' | 'resources' | 'operations' | 'tickets' | 'audit';
export type NavigationItem = { id: View; label: string; caption: string; icon: string };

export const navigation: NavigationItem[] = [
  { id: 'overview', label: 'Command Center', caption: 'Global posture', icon: '◫' },
  { id: 'providers', label: 'Provider Fabric', caption: 'Connections', icon: '⌁' },
  { id: 'resources', label: 'Resource Matrix', caption: 'Live inventory', icon: '◇' },
  { id: 'operations', label: 'Operation Stream', caption: 'Execution trace', icon: '↯' },
  { id: 'tickets', label: 'Support Desk', caption: 'Tickets & SLA', icon: '◇' },
  { id: 'audit', label: 'Audit Stream', caption: 'Immutable trail', icon: '≋' },
];

export function isView(value: string | undefined): value is View {
  return navigation.some((item) => item.id === value);
}

export function viewFromRoute(value: string | undefined): View {
  return isView(value) ? value : 'overview';
}

export function viewPath(view: View): string {
  return view === 'overview' ? '/' : `/${view}`;
}
