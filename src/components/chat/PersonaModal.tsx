import { RefreshCcw, X } from "lucide-react";
import type { RefObject } from "react";
import type { PlayerPersona, PlayerPersonaInput } from "../../tauri";

export type PersonaModalMode = "list" | "add" | "edit";

export function PersonaModal({
  activePersona,
  archivedPersonas,
  busy,
  form,
  listConfirmRequired,
  mode,
  modalRef,
  onArchive,
  onBackdropClose,
  onCancelForm,
  onClose,
  onConfirmList,
  onEdit,
  onOpenAdd,
  onRestore,
  onSave,
  onSelect,
  personas,
  setForm,
}: {
  activePersona: PlayerPersona | null;
  archivedPersonas: PlayerPersona[];
  busy: boolean;
  form: PlayerPersonaInput;
  listConfirmRequired: boolean;
  mode: PersonaModalMode;
  modalRef: RefObject<HTMLDivElement>;
  onArchive: (persona: PlayerPersona) => void;
  onBackdropClose: () => void;
  onCancelForm: () => void;
  onClose: () => void;
  onConfirmList: () => void;
  onEdit: (personaId: string) => void;
  onOpenAdd: () => void;
  onRestore: (persona: PlayerPersona) => void;
  onSave: () => void;
  onSelect: (personaId: string) => void;
  personas: PlayerPersona[];
  setForm: (updater: (current: PlayerPersonaInput) => PlayerPersonaInput) => void;
}) {
  const title = mode === "list" ? "Player Persona" : mode === "add" ? "Add Persona" : "Edit Persona";

  return (
    <section
      className="persona-modal-backdrop"
      role="presentation"
      onPointerDown={onBackdropClose}
    >
      <div
        className="persona-modal"
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="persona-modal-title"
        onPointerDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <span className="eyebrow">Persona</span>
            <h2 id="persona-modal-title">{title}</h2>
          </div>
          <button type="button" title="Close" onClick={onClose}>
            <X size={16} />
          </button>
        </header>
        {mode === "list" ? (
          <div className="persona-list">
            {personas.map((persona) => (
              <article
                key={persona.persona_id}
                className={activePersona?.persona_id === persona.persona_id ? "persona-row selected" : "persona-row"}
              >
                <div>
                  <strong>{persona.display_name}</strong>
                  <span>{persona.persona_id}</span>
                  <p>{persona.description}</p>
                </div>
                <div className="persona-row-actions">
                  <button type="button" onClick={() => onSelect(persona.persona_id)}>
                    Select
                  </button>
                  {!persona.is_builtin ? (
                    <button type="button" onClick={() => onEdit(persona.persona_id)}>
                      Edit
                    </button>
                  ) : null}
                  {!persona.is_builtin ? (
                    <button
                      type="button"
                      onClick={() => onArchive(persona)}
                      disabled={busy || activePersona?.persona_id === persona.persona_id}
                      title={
                        activePersona?.persona_id === persona.persona_id
                          ? "Select another persona before archiving this one"
                          : "Archive persona"
                      }
                    >
                      Archive
                    </button>
                  ) : null}
                </div>
              </article>
            ))}
            {archivedPersonas.length > 0 ? (
              <section className="compact-list library-list archived-resource-list" aria-label="Archived personas">
                <div className="list-section-heading">
                  <strong>Archived Personas</strong>
                  <span className="muted">{archivedPersonas.length}</span>
                </div>
                {archivedPersonas.map((persona) => (
                  <article key={persona.persona_id} className="soul-row archived-resource-row">
                    <span>{persona.display_name}</span>
                    <small>{persona.description || persona.persona_id}</small>
                    <button
                      type="button"
                      className="ghost-action"
                      onClick={() => onRestore(persona)}
                      disabled={busy}
                      title="Restore archived persona"
                    >
                      <RefreshCcw size={14} />
                      <span>Restore</span>
                    </button>
                  </article>
                ))}
              </section>
            ) : null}
            <div className="persona-list-actions">
              <button type="button" className="persona-add-button" onClick={onOpenAdd}>
                Add Persona
              </button>
              {listConfirmRequired ? (
                <div className="persona-form-actions">
                  <button type="button" className="persona-cancel-button" onClick={onClose}>
                    Cancel
                  </button>
                  <button type="button" className="persona-confirm-button" onClick={onConfirmList} disabled={!activePersona}>
                    Confirm Persona
                  </button>
                </div>
              ) : null}
            </div>
          </div>
        ) : (
          <div className="persona-form">
            <label>
              <span>Name</span>
              <input value={form.display_name} onChange={(event) => setForm((current) => ({ ...current, display_name: event.target.value }))} />
            </label>
            <label>
              <span>Gender code</span>
              <input value={form.gender_code} onChange={(event) => setForm((current) => ({ ...current, gender_code: event.target.value }))} />
            </label>
            <label>
              <span>Pronouns</span>
              <input value={form.pronouns} onChange={(event) => setForm((current) => ({ ...current, pronouns: event.target.value }))} />
            </label>
            <label>
              <span>Description</span>
              <textarea value={form.description} onChange={(event) => setForm((current) => ({ ...current, description: event.target.value }))} />
            </label>
            <label>
              <span>Appearance</span>
              <textarea value={form.appearance ?? ""} onChange={(event) => setForm((current) => ({ ...current, appearance: event.target.value }))} />
            </label>
            <label>
              <span>Notes</span>
              <textarea value={form.notes ?? ""} onChange={(event) => setForm((current) => ({ ...current, notes: event.target.value }))} />
            </label>
            <div className="persona-form-actions">
              <button type="button" onClick={onCancelForm}>
                Cancel
              </button>
              <button type="button" onClick={onSave}>
                Save
              </button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
