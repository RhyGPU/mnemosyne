import { FormEvent, useEffect, useRef, useState } from "react";
import { useModalBehavior } from "./a11y";

export type AppDialogMode = "alert" | "confirm" | "prompt" | "textarea";
export type AppDialogResult = boolean | string | null;

export type AppDialogState = {
  mode: AppDialogMode;
  title: string;
  message?: string;
  defaultValue?: string;
  placeholder?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  terminal?: boolean;
};

export function AppDialog({
  dialog,
  onResolve,
}: {
  dialog: AppDialogState | null;
  onResolve: (result: AppDialogResult) => void;
}) {
  const [value, setValue] = useState("");
  const panelRef = useRef<HTMLFormElement | null>(null);
  const isTextInput = dialog?.mode === "prompt" || dialog?.mode === "textarea";
  const resolveCancel = () => onResolve(dialog?.mode === "alert" ? true : null);
  const resolveConfirm = () => onResolve(isTextInput ? value : true);

  useEffect(() => {
    if (!dialog) return;
    setValue(dialog.defaultValue ?? "");
  }, [dialog]);

  useModalBehavior({
    active: Boolean(dialog),
    closeOnEscape: true,
    onClose: resolveCancel,
    panelRef,
  });

  if (!dialog) return null;

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    resolveConfirm();
  }

  return (
    <div
      className={`app-dialog-backdrop${dialog.terminal ? " terminal" : ""}`}
      role="presentation"
      onClick={() => {
        if (!dialog.destructive) resolveCancel();
      }}
      onMouseDown={() => {
        if (!dialog.destructive) resolveCancel();
      }}
      onPointerDown={() => {
        if (!dialog.destructive) resolveCancel();
      }}
    >
      <form
        ref={panelRef}
        className={`app-dialog${dialog.destructive ? " destructive" : ""}${dialog.terminal ? " terminal" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="app-dialog-title"
        onSubmit={handleSubmit}
        onClick={(event) => event.stopPropagation()}
        onMouseDown={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
      >
        <header>
          <span className="eyebrow">{dialog.terminal ? "Terminal confirm" : "Mnemosyne"}</span>
          <h2 id="app-dialog-title">{dialog.title}</h2>
        </header>
        {dialog.message ? <p>{dialog.message}</p> : null}
        {dialog.mode === "prompt" ? (
          <input value={value} placeholder={dialog.placeholder} onChange={(event) => setValue(event.target.value)} />
        ) : null}
        {dialog.mode === "textarea" ? (
          <textarea value={value} placeholder={dialog.placeholder} rows={7} onChange={(event) => setValue(event.target.value)} />
        ) : null}
        <footer>
          {dialog.mode !== "alert" ? (
            <button type="button" className="ghost-action" onClick={resolveCancel}>
              {dialog.cancelLabel ?? "Cancel"}
            </button>
          ) : null}
          <button type="submit" className={dialog.destructive ? "ghost-action purge" : "start-chat-button"}>
            {dialog.confirmLabel ?? (dialog.mode === "alert" ? "Close" : "Confirm")}
          </button>
        </footer>
      </form>
    </div>
  );
}
