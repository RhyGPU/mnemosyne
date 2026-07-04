import { RefObject, useEffect } from "react";

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not(:disabled)",
  "input:not(:disabled)",
  "select:not(:disabled)",
  "textarea:not(:disabled)",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

export function getFocusableElements(container: HTMLElement | null) {
  if (!container) return [];
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => !element.hasAttribute("disabled") && element.getAttribute("aria-hidden") !== "true",
  );
}

export function focusFirstElement(container: HTMLElement | null) {
  const [first] = getFocusableElements(container);
  first?.focus();
}

export function trapTabKey(event: Pick<KeyboardEvent, "key" | "shiftKey" | "preventDefault">, container: HTMLElement | null) {
  if (event.key !== "Tab") return;
  const focusables = getFocusableElements(container);
  if (focusables.length <= 1) {
    event.preventDefault();
    focusables[0]?.focus();
    return;
  }
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

export function useModalBehavior({
  active,
  closeOnEscape = true,
  onClose,
  panelRef,
  restoreFocus = true,
}: {
  active: boolean;
  closeOnEscape?: boolean;
  onClose: () => void;
  panelRef: RefObject<HTMLElement>;
  restoreFocus?: boolean;
}) {
  useEffect(() => {
    if (!active) return;
    const previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const frame = window.requestAnimationFrame(() => focusFirstElement(panelRef.current));
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && closeOnEscape) {
        event.preventDefault();
        onClose();
        return;
      }
      trapTabKey(event, panelRef.current);
    };
    document.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.cancelAnimationFrame(frame);
      document.removeEventListener("keydown", handleKeyDown, true);
      if (restoreFocus) previouslyFocused?.focus();
    };
  }, [active, closeOnEscape, onClose, panelRef, restoreFocus]);
}
