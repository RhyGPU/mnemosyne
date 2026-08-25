import { useRef, useState } from "react";
import type {
  AppDialogResult,
  AppDialogState,
} from "../components/dialogs";

export function useAppDialogs() {
  const [dialog, setDialog] = useState<AppDialogState | null>(null);
  const resolverRef = useRef<((result: AppDialogResult) => void) | null>(null);

  function open(nextDialog: AppDialogState): Promise<AppDialogResult> {
    return new Promise((resolve) => {
      resolverRef.current = resolve;
      setDialog(nextDialog);
    });
  }

  function resolve(result: AppDialogResult) {
    const resolver = resolverRef.current;
    resolverRef.current = null;
    setDialog(null);
    resolver?.(result);
  }

  async function alert(title: string, message?: string, terminal = false) {
    await open({ mode: "alert", title, message, terminal });
  }

  async function confirm(
    title: string,
    message?: string,
    destructive = false,
    terminal = false,
  ) {
    return (
      (await open({
        mode: "confirm",
        title,
        message,
        destructive,
        terminal,
        confirmLabel: destructive ? "Confirm" : "Continue",
      })) === true
    );
  }

  async function prompt(
    title: string,
    defaultValue = "",
    options: {
      message?: string;
      textarea?: boolean;
      placeholder?: string;
      confirmLabel?: string;
    } = {},
  ) {
    const result = await open({
      mode: options.textarea ? "textarea" : "prompt",
      title,
      message: options.message,
      defaultValue,
      placeholder: options.placeholder,
      confirmLabel: options.confirmLabel,
    });
    return typeof result === "string" ? result : null;
  }

  return {
    dialog,
    resolve,
    alert,
    confirm,
    prompt,
  };
}
