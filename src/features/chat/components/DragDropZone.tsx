import { Paperclip } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { type DragEvent, memo, type ReactNode, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { cn } from '@/shared/utils/cn';

interface DragDropZoneProps {
  children: ReactNode;
  onImageDrop: (base64: string) => void;
  onTextDrop: (content: string, filename: string) => void;
}

export const DragDropZone = memo<DragDropZoneProps>(({ children, onImageDrop, onTextDrop }) => {
  const { t } = useTranslation();
  const [isDragActive, setIsDragActive] = useState(false);

  const handleDrag = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setIsDragActive(true);
    } else if (e.type === 'dragleave') {
      setIsDragActive(false);
    }
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragActive(false);

      const file = e.dataTransfer.files[0];
      if (!file) return;

      const MAX_SIZE = 5 * 1024 * 1024;
      if (file.size > MAX_SIZE) {
        toast.error('File too large (max 5MB)');
        return;
      }

      const reader = new FileReader();
      if (file.type.startsWith('image/')) {
        reader.onload = (event) => {
          const result = event.target?.result;
          if (typeof result === 'string') onImageDrop(result);
        };
        reader.readAsDataURL(file);
      } else {
        reader.onload = (event) => {
          const result = event.target?.result;
          if (typeof result === 'string') {
            onTextDrop(result.substring(0, 20_000), file.name);
          }
        };
        reader.readAsText(file);
      }
    },
    [onImageDrop, onTextDrop],
  );

  return (
    <section
      aria-label={t('chat.fileDropZone', 'File drop zone')}
      className="flex flex-col w-full h-full min-h-0 relative"
      onDragEnter={handleDrag}
      onDragLeave={handleDrag}
      onDragOver={handleDrag}
      onDrop={handleDrop}
    >
      {/* Drop overlay */}
      <AnimatePresence>
        {isDragActive && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className={cn(
              'absolute inset-0 z-50',
              'bg-black/80 backdrop-blur-sm',
              'flex items-center justify-center',
              'border-4 border-[var(--matrix-accent)] border-dashed rounded-xl',
              'pointer-events-none',
            )}
          >
            <div className="text-[var(--matrix-accent)] text-2xl font-mono animate-pulse flex flex-col items-center gap-4">
              <Paperclip size={64} />
              <span>{t('chat.dropFileToAddContext', 'DROP FILE TO ADD CONTEXT')}</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      {children}
    </section>
  );
});

DragDropZone.displayName = 'DragDropZone';
