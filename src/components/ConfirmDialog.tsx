import { useCallback, useEffect, useRef } from "react";
import type { GlassButtonVariant } from "./GlassButton";
import { GlassButton } from "./GlassButton";
import "./ConfirmDialog.css";

// ── Types ──────────────────────────────────────────────────────────────────

type ConfirmDialogProps = {
	/** Whether the dialog is visible. */
	open: boolean;
	/** Dialog title text. */
	title: string;
	/** Dialog body message. */
	message: string;
	/** Label for the confirm button. Default: "Confirm". */
	confirmLabel?: string;
	/** Visual variant for the confirm button. Default: "error". */
	confirmVariant?: GlassButtonVariant;
	/** Whether the confirm button shows a loading spinner. */
	confirmLoading?: boolean;
	/** Called when the user confirms. */
	onConfirm: () => void;
	/** Called when the user cancels (Cancel button, Esc key, or overlay click). */
	onCancel: () => void;
};

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Reusable modal confirmation dialog with blur overlay.
 *
 * Uses the Liquid Glass 4-layer sandwich for the dialog card.
 * Positioned absolute inside the popover (not a Portal).
 *
 * Keyboard:
 *   - Esc → cancel (with stopPropagation to prevent popover hide)
 *   - Enter → confirm
 *
 * Accessibility:
 *   - role="alertdialog", aria-modal="true"
 *   - aria-labelledby + aria-describedby
 *   - Auto-focuses cancel button on open
 */
export function ConfirmDialog({
	open,
	title,
	message,
	confirmLabel = "Confirm",
	confirmVariant = "error",
	confirmLoading = false,
	onConfirm,
	onCancel,
}: ConfirmDialogProps) {
	const cancelButtonRef = useRef<HTMLDivElement>(null);

	// Focus the cancel button when dialog opens
	useEffect(() => {
		if (open && cancelButtonRef.current) {
			const button = cancelButtonRef.current.querySelector("button");
			button?.focus();
		}
	}, [open]);

	// Handle keyboard events on the overlay
	const handleKeyDown = useCallback(
		(event: React.KeyboardEvent) => {
			if (event.key === "Escape") {
				event.stopPropagation();
				onCancel();
			} else if (event.key === "Enter") {
				event.stopPropagation();
				if (!confirmLoading) {
					onConfirm();
				}
			}
		},
		[onCancel, onConfirm, confirmLoading],
	);

	// Handle overlay click (click outside dialog to dismiss)
	const handleOverlayClick = useCallback(
		(event: React.MouseEvent) => {
			if (event.target === event.currentTarget) {
				onCancel();
			}
		},
		[onCancel],
	);

	if (!open) return null;

	return (
		<div
			className="confirm-dialog-overlay"
			onClick={handleOverlayClick}
			onKeyDown={handleKeyDown}
			role="alertdialog"
			aria-modal="true"
			aria-labelledby="confirm-dialog-title"
			aria-describedby="confirm-dialog-message"
		>
			<div className="confirm-dialog liquidGlass-wrapper">
				<div className="liquidGlass-effect" />
				<div className="liquidGlass-tint" />
				<div className="liquidGlass-shine" />
				<div className="liquidGlass-text confirm-dialog__content">
					<h2 className="confirm-dialog__title" id="confirm-dialog-title">
						{title}
					</h2>
					<p className="confirm-dialog__message" id="confirm-dialog-message">
						{message}
					</p>
					<div className="confirm-dialog__actions">
						<div ref={cancelButtonRef}>
							<GlassButton variant="neutral" onClick={onCancel}>
								Cancel
							</GlassButton>
						</div>
						<GlassButton
							variant={confirmVariant}
							onClick={onConfirm}
							loading={confirmLoading}
						>
							{confirmLabel}
						</GlassButton>
					</div>
				</div>
			</div>
		</div>
	);
}
