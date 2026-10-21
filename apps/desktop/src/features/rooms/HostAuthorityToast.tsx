import React from 'react';
import { Lock } from 'lucide-react';

export interface HostAuthorityToastProps {
  message: string | null;
}

export function HostAuthorityToast({ message }: HostAuthorityToastProps) {
  if (!message) return null;

  return (
    <div
      data-testid="host-authority-toast"
      className="pointer-events-none fixed top-16 inset-x-0 z-50 flex items-center justify-center animate-in fade-in slide-in-from-top-2 duration-200"
    >
      <div className="flex items-center gap-2 px-4 py-2 rounded-xl bg-zinc-900/90 text-amber-300 text-xs sm:text-sm font-medium border border-amber-500/40 shadow-2xl backdrop-blur-md">
        <Lock className="w-4 h-4 text-amber-400 shrink-0" />
        <span>{message}</span>
      </div>
    </div>
  );
}
