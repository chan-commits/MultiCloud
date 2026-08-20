export type StoredSession = {
  token: string;
  expiresAt: string;
  isPlatformAdmin?: boolean;
};

const sessionKey = 'multicloud.session';
const organizationKey = 'multicloud.organization';

export function readSession(): StoredSession | null {
  const stored = sessionStorage.getItem(sessionKey);
  if (!stored) return null;
  try {
    const session = JSON.parse(stored) as StoredSession;
    if (new Date(session.expiresAt) <= new Date()) {
      clearSession();
      return null;
    }
    return session;
  } catch {
    clearSession();
    return null;
  }
}

export function persistSession(session: StoredSession): void {
  sessionStorage.setItem(sessionKey, JSON.stringify(session));
}

export function clearSession(): void {
  sessionStorage.removeItem(sessionKey);
  localStorage.removeItem(organizationKey);
}

export function readPreferredOrganization(): string | null {
  return localStorage.getItem(organizationKey);
}

export function persistOrganization(id: string): void {
  localStorage.setItem(organizationKey, id);
}
