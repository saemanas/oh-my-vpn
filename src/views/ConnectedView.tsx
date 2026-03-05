import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ErrorCard } from "../components/ErrorCard";
import { GlassButton } from "../components/GlassButton";
import { SessionCard } from "../components/SessionCard";
import { useNavigation } from "../navigation/stack-context";
import type { AppError, SessionStatus } from "../types/ipc";
import "./ConnectedView.css";

// ── Types ──────────────────────────────────────────────────────────────────

type ConnectedViewProps = {
	/** Initial session data passed from session check or connect flow. */
	initialSession: SessionStatus;
};

// ── Constants ──────────────────────────────────────────────────────────────

/** Polling interval for session status updates (milliseconds). */
const POLL_INTERVAL_MS = 1000;

// ── Component ──────────────────────────────────────────────────────────────

/**
 * ConnectedView -- displayed when a VPN session is active.
 *
 * Responsibilities:
 *   - Poll `get_session_status` every 1 second to update elapsed time and cost
 *   - Display session info via SessionCard
 *   - Handle disconnect via `disconnect` IPC command
 *   - Navigate back to DisconnectedView on successful disconnect
 *   - Surface disconnect errors inline with retry
 */
export function ConnectedView({ initialSession }: ConnectedViewProps) {
	const { pop } = useNavigation();
	const [session, setSession] = useState<SessionStatus>(initialSession);
	const [isDisconnecting, setIsDisconnecting] = useState(false);
	const [disconnectError, setDisconnectError] = useState<AppError | null>(null);
	const [showConfirm, setShowConfirm] = useState(false);
	const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

	// ── Polling ────────────────────────────────────────────────────────────

	useEffect(() => {
		async function pollStatus() {
			try {
				const status = await invoke<SessionStatus | null>(
					"get_session_status",
				);
				if (status) {
					setSession(status);
				}
			} catch {
				// Polling failures are silent -- the last known state remains displayed.
				// Disconnect or app restart will resolve stale state.
			}
		}

		intervalRef.current = setInterval(() => void pollStatus(), POLL_INTERVAL_MS);

		return () => {
			if (intervalRef.current !== null) {
				clearInterval(intervalRef.current);
				intervalRef.current = null;
			}
		};
	}, []);

	// ── Disconnect ─────────────────────────────────────────────────────────

	const handleDisconnectRequest = useCallback(() => {
		setShowConfirm(true);
	}, []);

	const handleConfirmCancel = useCallback(() => {
		setShowConfirm(false);
	}, []);

	const handleConfirmDisconnect = useCallback(async () => {
		setIsDisconnecting(true);
		setDisconnectError(null);

		// Stop polling while disconnecting
		if (intervalRef.current !== null) {
			clearInterval(intervalRef.current);
			intervalRef.current = null;
		}

		try {
			await invoke("disconnect");
			pop();
		} catch (err: unknown) {
			const appError = err as AppError;
			setDisconnectError(appError);
			setIsDisconnecting(false);
			setShowConfirm(false);
		}
	}, [pop]);

	const handleRetry = useCallback(() => {
		setDisconnectError(null);
		setShowConfirm(true);
	}, []);

	// ── Derived state ──────────────────────────────────────────────────────

	const isDestructionFailure =
		disconnectError?.code === "PROVIDER_DESTRUCTION_FAILED";
	const consoleUrl =
		isDestructionFailure && disconnectError?.details
			? String((disconnectError.details as { consoleUrl?: string }).consoleUrl ?? "")
			: "";

	// ── Render ─────────────────────────────────────────────────────────────

	return (
		<div className="connected-view">
			{/* Status badge */}
			<div className="connected-view__status">
				<span className="connected-view__dot" aria-hidden="true" />
				<span className="connected-view__label">CONNECTED</span>
			</div>

			{/* Session card */}
			<SessionCard session={session} />

			{/* Disconnect error */}
			{disconnectError && !isDestructionFailure && (
				<ErrorCard message={disconnectError.message}>
					<GlassButton variant="neutral" onClick={handleRetry}>
						Retry
					</GlassButton>
				</ErrorCard>
			)}

			{/* Persistent destruction failure -- manual cleanup needed */}
			{disconnectError && isDestructionFailure && (
				<ErrorCard
					message={disconnectError.message}
					description="Delete the server manually from your provider console."
					variant="warning"
				>
					{consoleUrl && (
						<GlassButton
							variant="warning"
							onClick={() =>
								window.open(consoleUrl, "_blank", "noopener")
							}
						>
							Open Console
						</GlassButton>
					)}
				</ErrorCard>
			)}

			{/* Disconnect button */}
			{!disconnectError && (
				<GlassButton
					variant="error"
					onClick={handleDisconnectRequest}
				>
					Disconnect
				</GlassButton>
			)}

			{/* Confirm dialog */}
			<ConfirmDialog
				open={showConfirm}
				title="Disconnect"
				message="Server will be destroyed. Continue?"
				confirmLabel="Destroy"
				confirmVariant="error"
				confirmLoading={isDisconnecting}
				onConfirm={() => void handleConfirmDisconnect()}
				onCancel={handleConfirmCancel}
			/>
		</div>
	);
}
