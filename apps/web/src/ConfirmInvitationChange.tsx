import { useEffect, useId, useRef } from "react";

export function ConfirmInvitationChange({
  action,
  description,
  danger,
  onCancel,
  onConfirm,
}: {
  action: string;
  description: string;
  danger: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const title = useId();
  useEffect(() => {
    const dialog = ref.current!;
    dialog.showModal();
    return () => dialog.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className="admin-room-dialog"
      aria-labelledby={title}
      aria-describedby={title + "-detail"}
      onCancel={onCancel}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          onConfirm();
        }}
      >
        <h2 id={title}>{action}?</h2>
        <p id={title + "-detail"}>{description}</p>
        <div className="admin-dialog-actions">
          <button
            type="button"
            className="button secondary"
            autoFocus
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            className={danger ? "button admin-danger" : "button admin-primary"}
          >
            {action}
          </button>
        </div>
      </form>
    </dialog>
  );
}
