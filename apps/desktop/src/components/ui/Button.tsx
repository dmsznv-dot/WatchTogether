import React, { ButtonHTMLAttributes, forwardRef } from 'react';
import { cn } from '../../utils/cn';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md' | 'lg' | 'icon';
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'secondary', size = 'md', children, disabled, ...props }, ref) => {
    const baseClasses =
      'inline-flex items-center justify-center font-medium transition-all focus:outline-none select-none disabled:opacity-40 disabled:cursor-not-allowed';

    const variantClasses = {
      primary:
        'bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg shadow-indigo-600/30 border border-indigo-500/30 active:scale-[0.98]',
      secondary:
        'bg-zinc-800/80 hover:bg-zinc-700/80 text-zinc-200 border border-zinc-700/60 active:scale-[0.98]',
      ghost:
        'bg-transparent hover:bg-white/10 text-zinc-300 hover:text-white',
      danger:
        'bg-rose-600 hover:bg-rose-500 text-white shadow-lg shadow-rose-600/30 border border-rose-500/30 active:scale-[0.98]',
    };

    const sizeClasses = {
      sm: 'text-xs px-2.5 py-1 rounded-md gap-1.5',
      md: 'text-sm px-3.5 py-1.5 rounded-lg gap-2',
      lg: 'text-base px-5 py-2.5 rounded-xl gap-2.5',
      icon: 'p-2 rounded-lg',
    };

    return (
      <button
        ref={ref}
        disabled={disabled}
        className={cn(baseClasses, variantClasses[variant], sizeClasses[size], className)}
        {...props}
      >
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';
