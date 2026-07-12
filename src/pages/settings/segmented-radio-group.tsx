import type { KeyboardEvent, ReactNode } from "react";

import { cn } from "@/lib/utils";

type SegmentedRadioGroupProps = {
  readonly label: string;
  readonly className?: string;
  readonly children: ReactNode;
};

export function SegmentedRadioGroup({
  label,
  className,
  children,
}: SegmentedRadioGroupProps) {
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = event.target instanceof HTMLElement
      ? event.target.closest<HTMLElement>('[role="radio"]')
      : null;
    if (!current || !event.currentTarget.contains(current)) return;

    const options = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>(
        '[role="radio"]:not([aria-disabled="true"])',
      ),
    );
    const currentIndex = options.indexOf(current);
    if (currentIndex === -1 || options.length === 0) return;

    let nextIndex: number;
    switch (event.key) {
      case "ArrowRight":
      case "ArrowDown":
        nextIndex = (currentIndex + 1) % options.length;
        break;
      case "ArrowLeft":
      case "ArrowUp":
        nextIndex = (currentIndex - 1 + options.length) % options.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = options.length - 1;
        break;
      default:
        return;
    }

    event.preventDefault();
    options[nextIndex].focus();
    options[nextIndex].click();
  };

  return (
    <div
      className={cn("flex gap-1", className)}
      role="radiogroup"
      aria-label={label}
      onKeyDown={handleKeyDown}
    >
      {children}
    </div>
  );
}
