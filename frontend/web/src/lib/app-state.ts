import type { Operation, Resource } from './api';

export function activeResourcesOf(resources: Resource[]): Resource[] {
  return resources.filter((item) => item.lifecycle === 'active');
}

export function driftedResourcesOf(resources: Resource[]): Resource[] {
  return resources.filter(
    (item) =>
      item.desired_state &&
      item.observed_state &&
      JSON.stringify(item.desired_state.state) !== JSON.stringify(item.observed_state.state),
  );
}

export function runningOperationsOf(operations: Operation[]): Operation[] {
  return operations.filter((item) => ['queued', 'running', 'retrying'].includes(item.status));
}

export function failedOperationsOf(operations: Operation[]): Operation[] {
  return operations.filter((item) => item.status === 'failed');
}
