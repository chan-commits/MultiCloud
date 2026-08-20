import type { Operation, ProviderAccount, Resource } from './api';
import { ApiClient } from './api';

export function syncResourceType(provider: ProviderAccount): 'dns_zone' | 'compute_instance' {
  return provider.provider_kind === 'cloudflare' ? 'dns_zone' : 'compute_instance';
}

export function resourceProvider(
  providers: ProviderAccount[],
  resource: Resource,
): ProviderAccount | null {
  return providers.find((item) => item.id === resource.provider_account_id) ?? null;
}

export function providerTest(client: ApiClient, provider: ProviderAccount) {
  return client.testProvider(provider.id);
}

export function providerSync(client: ApiClient, provider: ProviderAccount) {
  return client.syncProvider(provider.id, syncResourceType(provider));
}

export function resourceOperation(
  client: ApiClient,
  provider: ProviderAccount,
  resource: Resource,
  action: string,
) {
  return client.runProviderOperation(provider.id, action, resource.external_id ?? '');
}

export function cancelOperation(client: ApiClient, operation: Operation) {
  return client.cancelOperation(operation.id);
}
