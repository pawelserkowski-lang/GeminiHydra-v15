import { Code2, FileSearch, FolderTree, GitBranch, Globe, Image, MessageSquare } from 'lucide-react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '@/components/molecules/EmptyState';
import { type PromptSuggestion, PromptSuggestions } from '@/components/molecules/PromptSuggestions';

const GH_SUGGESTIONS: PromptSuggestion[] = [
  { labelKey: 'chat.suggestions.analyzeCode', fallback: 'Analyze the code structure of my project', icon: Code2 },
  { labelKey: 'chat.suggestions.readFile', fallback: 'Read and explain a file from my codebase', icon: FileSearch },
  { labelKey: 'chat.suggestions.gitStatus', fallback: 'Show git status and recent commits', icon: GitBranch },
  { labelKey: 'chat.suggestions.scrapeWebpage', fallback: 'Fetch and summarize a webpage', icon: Globe },
  { labelKey: 'chat.suggestions.analyzeImage', fallback: 'Analyze an image and describe what you see', icon: Image },
  {
    labelKey: 'chat.suggestions.exploreDirectory',
    fallback: 'Explore a directory and summarize contents',
    icon: FolderTree,
  },
];

interface ChatEmptyStateProps {
  onSuggestionSelect: (text: string) => void;
}

export const ChatEmptyState = memo<ChatEmptyStateProps>(({ onSuggestionSelect }) => {
  const { t } = useTranslation();
  return (
    <div className="h-full flex flex-col items-center justify-center">
      <EmptyState
        icon={MessageSquare}
        title={t('chat.emptyState', 'Start a conversation')}
        description={t('chat.emptyStateDesc', 'Type a message or drop a file to begin.')}
      />
      <PromptSuggestions suggestions={GH_SUGGESTIONS} onSelect={onSuggestionSelect} />
    </div>
  );
});

ChatEmptyState.displayName = 'ChatEmptyState';
