import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { GlassButton } from "../components/GlassButton";
import { useNavigation } from "../navigation/stack-context";
import type { Provider, ProviderInfo, ProviderStatus } from "../types/ipc";
import { ProviderSelection } from "./ProviderSelection";
import "./ProviderManagementView.css";

// ── Helpers ────────────────────────────────────────────────────────────────

/** Maps provider enum to display name. */
function formatProviderName(provider: Provider): string {
	const names: Record<Provider, string> = {
		hetzner: "Hetzner",
		aws: "AWS",
		gcp: "GCP",
	};
	return names[provider];
}

/** Maps provider status to a human-readable label. */
function formatStatusLabel(status: ProviderStatus): string {
	const labels: Record<ProviderStatus, string> = {
		valid: "Valid",
		invalid: "Invalid",
		unchecked: "Unchecked",
	};
	return labels[status];
}

// ── Component ──────────────────────────────────────────────────────────────

/**
 * ProviderManagementView -- settings panel for managing registered cloud providers.
 *
 * Responsibilities:
 *   - Fetch all registered providers on mount (list_providers IPC)
 *   - Display each provider with name, account label, and status badge
 *   - Allow removal via ConfirmDialog + remove_provider IPC
 *   - Navigate to ProviderSelection to add a new provider
 *   - Surface inline errors with Retry action
 *   - Show empty state when no providers are registered
 */
export function ProviderManagementView() {
	const { push } = useNavigation();

	// ── State ──────────────────────────────────────────────────────────────

	const [providers, setProviders] = useState<ProviderInfo[]>([]);
	const [isLoading, setIsLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);
	const [confirmRemoveProvider, setConfirmRemoveProvider] =
		useState<Provider | null>(null);
	const [isRemoving, setIsRemoving] = useState(false);

	// ── IPC: list_providers ────────────────────────────────────────────────

	const fetchProviders = useCallback(async () => {
		setIsLoading(true);
		setError(null);

		try {
			const result = await invoke<ProviderInfo[]>("list_providers");
			setProviders(result);
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			setError(message);
		} finally {
			setIsLoading(false);
		}
	}, []);

	// ── IPC: remove_provider ───────────────────────────────────────────────

	const handleConfirmRemove = useCallback(async () => {
		if (!confirmRemoveProvider) return;

		setIsRemoving(true);
		try {
			await invoke("remove_provider", { provider: confirmRemoveProvider });
			setConfirmRemoveProvider(null);
			await fetchProviders();
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			setError(message);
			setConfirmRemoveProvider(null);
		} finally {
			setIsRemoving(false);
		}
	}, [confirmRemoveProvider, fetchProviders]);

	// ── Navigation ─────────────────────────────────────────────────────────

	const handleAddProvider = useCallback(() => {
		push("provider-selection", "Add Provider", <ProviderSelection />);
	}, [push]);

	// ── Handlers ───────────────────────────────────────────────────────────

	const handleRemoveClick = useCallback((provider: Provider) => {
		setConfirmRemoveProvider(provider);
	}, []);

	const handleCancelRemove = useCallback(() => {
		setConfirmRemoveProvider(null);
	}, []);

	// ── Effects ────────────────────────────────────────────────────────────

	useEffect(() => {
		void fetchProviders();
	}, [fetchProviders]);

	// ── Render: error ──────────────────────────────────────────────────────

	if (error && !isLoading) {
		return (
			<div className="provider-management-view provider-management-view--error">
				<p className="provider-management-error__message" role="alert">
					Could not load providers: {error}
				</p>
				<GlassButton variant="neutral" onClick={() => void fetchProviders()}>
					Retry
				</GlassButton>
			</div>
		);
	}

	// ── Render: loading ────────────────────────────────────────────────────

	if (isLoading) {
		return (
			<div className="provider-management-view provider-management-view--loading">
				<div className="provider-management-skeleton">
					<div className="provider-management-skeleton__row" />
					<div className="provider-management-skeleton__row" />
				</div>
			</div>
		);
	}

	// ── Render: empty ──────────────────────────────────────────────────────

	if (providers.length === 0) {
		return (
			<div className="provider-management-view provider-management-view--empty">
				<p className="provider-management-empty__message">
					No cloud providers configured. Add credentials to get started.
				</p>
				<GlassButton variant="success" onClick={handleAddProvider}>
					Add Provider
				</GlassButton>
			</div>
		);
	}

	// ── Render: main ───────────────────────────────────────────────────────

	return (
		<div className="provider-management-view">
			{/* Provider list */}
			<ul
				className="provider-management-list"
				aria-label="Registered providers"
			>
				{providers.map((info) => (
					<li
						key={info.provider}
						className="liquidGlass-wrapper provider-managed-row"
					>
						<div className="liquidGlass-effect" />
						<div className="liquidGlass-tint" />
						<div className="liquidGlass-shine" />
						<div className="liquidGlass-text provider-managed-row__content">
							{/* Left: name + account */}
							<span className="provider-managed-row__info">
								<span className="provider-managed-row__name">
									{formatProviderName(info.provider)}
								</span>
								<span
									className="provider-managed-row__separator"
									aria-hidden="true"
								>
									·
								</span>
								<span className="provider-managed-row__account">
									{info.accountLabel}
								</span>
							</span>

							{/* Center: status badge */}
							<span
								className={`provider-managed-row__badge provider-managed-row__badge--${info.status}`}
							>
								{formatStatusLabel(info.status)}
							</span>

							{/* Right: remove button */}
							<GlassButton
								variant="error"
								onClick={() => handleRemoveClick(info.provider)}
								aria-label={`Remove ${formatProviderName(info.provider)}`}
							>
								Remove
							</GlassButton>
						</div>
					</li>
				))}
			</ul>

			{/* Add Provider button */}
			<GlassButton variant="success" onClick={handleAddProvider}>
				Add Provider
			</GlassButton>

			{/* Removal confirmation dialog */}
			<ConfirmDialog
				open={confirmRemoveProvider !== null}
				title="Remove Provider"
				message={
					confirmRemoveProvider
						? `Remove ${formatProviderName(confirmRemoveProvider)}? This will delete the stored credentials.`
						: ""
				}
				confirmLabel="Remove"
				confirmVariant="error"
				confirmLoading={isRemoving}
				onConfirm={() => void handleConfirmRemove()}
				onCancel={handleCancelRemove}
			/>
		</div>
	);
}
