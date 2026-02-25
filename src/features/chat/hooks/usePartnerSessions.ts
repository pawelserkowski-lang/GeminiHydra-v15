import { useQuery } from '@tanstack/react-query';
import { fetchPartnerSession, fetchPartnerSessions } from '@/shared/api/partnerClient';

export function usePartnerSessions() {
  return useQuery({
    queryKey: ['partner-sessions'],
    queryFn: fetchPartnerSessions,
    refetchInterval: 30_000,
    retry: 1,
    staleTime: 15_000,
  });
}

export function usePartnerSession(id: string | null) {
  return useQuery({
    queryKey: ['partner-session', id],
    queryFn: () => fetchPartnerSession(id as string),
    enabled: !!id,
    retry: 1,
    staleTime: 60_000,
  });
}
