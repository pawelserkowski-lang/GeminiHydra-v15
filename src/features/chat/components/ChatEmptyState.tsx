import { MessageSquare } from 'lucide-react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '@/components/molecules/EmptyState';

export const ChatEmptyState = memo(() => {
  const { t } = useTranslation();
  return (
    <EmptyState
      icon={MessageSquare}
      title={t('chat.emptyState', 'Start a conversation')}
      description={t('chat.emptyStateDesc', 'Type a message or drop a file to begin.')}
      className="h-full"
    />
  );
});

ChatEmptyState.displayName = 'ChatEmptyState';
