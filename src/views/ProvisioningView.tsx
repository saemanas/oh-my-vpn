import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { ErrorCard } from "../components/ErrorCard";
import { GlassButton } from "../components/GlassButton";
import { ProvisioningStepper } from "../components/ProvisioningStepper";
import { useNavigation } from "../navigation/stack-context";
import type {
	AppError,
	ConnectProgress,
	Provider,
	SessionStatus,
} from "../types/ipc";
import { ConnectedView } from "./ConnectedView";
import "./ProvisioningView.css";

// ── Types ──────────────────────────────────────────────────────────────────

type ProvisioningViewProps = {
	/** Cloud provider for this connection attempt. */
	provider: Provider;
	/** Region display name, e.g. "Frankfurt · Hetzner". */
	regionLabel: string;
	/** Cloud region code passed to the connect IPC, e.g. "fsn1". */
	regionCode: string;
};

// ── Error mapping ──────────────────────────────────────────────────────────

/**
 * Map an AppError code to the stepper step that failed.
 *
 * - PROVIDER_PROVISIONING_FAILED → step 1 (Creating server)
 * - TUNNEL_SETUP_FAILED → step 3 (Connecting tunnel)
 * - All others → step 1 (pre-provisioning failure)
 */
function mapErrorToStep(code: string): number {
	switch (code) {
		case "PROVIDER_PROVISIONING_FAILED":
			return 1;
		case "TUNNEL_SETUP_FAILED":
			return 3;
		default:
			return 1;
	}
}

// ── Component ──────────────────────────────────────────────────────────────

/**
 * ProvisioningView -- orchestrates the connect flow with live progress.
 *
 * On mount:
 *   1. Subscribes to "connect-progress" backend events
 *   2. Invokes "connect" IPC
 *   3. Updates ProvisioningStepper on each progress event
 *   4. On success → pushes ConnectedView
 *   5. On failure → shows error card with Cancel / Retry
 *
 * Cleans up the event listener on unmount.
 */
export function ProvisioningView({
	provider,
	regionLabel,
	regionCode,
}: ProvisioningViewProps) {
	const { push, pop } = useNavigation();

	// ── State ──────────────────────────────────────────────────────────────

	const [currentStep, setCurrentStep] = useState(0);
	const [failedStep, setFailedStep] = useState<number | null>(null);
	const [errorMessage, setErrorMessage] = useState<string | null>(null);
	const [isConnecting, setIsConnecting] = useState(false);

	// Track unmount to prevent state updates after cleanup
	const unmountedRef = useRef(false);

	useEffect(() => {
		return () => {
			unmountedRef.current = true;
		};
	}, []);

	// ── Connect flow ───────────────────────────────────────────────────────

	const startConnect = useCallback(async () => {
		if (unmountedRef.current) return;

		// Reset state for retry
		setCurrentStep(0);
		setFailedStep(null);
		setErrorMessage(null);
		setIsConnecting(true);

		// Listen for progress events
		const unlisten = await listen<ConnectProgress>(
			"connect-progress",
			(event) => {
				if (!unmountedRef.current) {
					setCurrentStep(event.payload.step);
				}
			},
		);

		try {
			const session = await invoke<SessionStatus>("connect", {
				provider,
				region: regionCode,
			});

			if (!unmountedRef.current) {
				// All steps complete -- push connected view
				push(
					"connected",
					"Connected",
					<ConnectedView initialSession={session} />,
				);
			}
		} catch (err: unknown) {
			if (!unmountedRef.current) {
				const appError = err as AppError;
				const step = mapErrorToStep(appError.code ?? "");
				setCurrentStep(step);
				setFailedStep(step);
				setErrorMessage(appError.message ?? String(err));
				setIsConnecting(false);
			}
		} finally {
			unlisten();
		}
	}, [provider, regionCode, push]);

	// ── Auto-start on mount ────────────────────────────────────────────────

	const startConnectRef = useRef(startConnect);
	startConnectRef.current = startConnect;

	useEffect(() => {
		void startConnectRef.current();
	}, []);

	// ── Handlers ───────────────────────────────────────────────────────────

	const handleCancel = useCallback(() => {
		pop();
	}, [pop]);

	const handleRetry = useCallback(() => {
		void startConnect();
	}, [startConnect]);

	// ── Derived state ──────────────────────────────────────────────────────

	const hasFailed = failedStep !== null;
	const statusText = hasFailed ? "PROVISIONING FAILED" : "PROVISIONING...";
	const statusDotClass = hasFailed
		? "provisioning-status-dot--error"
		: "provisioning-status-dot--active";

	// ── Render ─────────────────────────────────────────────────────────────

	return (
		<div className="provisioning-view">
			{/* Status badge */}
			<div className="provisioning-status">
				<span className={`provisioning-status-dot ${statusDotClass}`} />
				<span className="provisioning-status-text">{statusText}</span>
			</div>

			{/* Region info */}
			<p className="provisioning-region">{regionLabel}</p>

			{/* Stepper */}
			<ProvisioningStepper currentStep={currentStep} failedStep={failedStep} />

			{/* Error card with actions */}
			{hasFailed && (
				<ErrorCard message={errorMessage ?? ""}>
					<GlassButton variant="neutral" onClick={handleCancel}>
						Cancel
					</GlassButton>
					<GlassButton variant="warning" onClick={handleRetry}>
						Retry
					</GlassButton>
				</ErrorCard>
			)}

			{/* Cancel button during active provisioning */}
			{!hasFailed && isConnecting && (
				<GlassButton variant="neutral" onClick={handleCancel}>
					Cancel
				</GlassButton>
			)}
		</div>
	);
}
