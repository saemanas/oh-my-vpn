import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { GlassButton } from "../components/GlassButton";
import { ProviderSelector } from "../components/ProviderSelector";
import { RegionList } from "../components/RegionList";
import { useNavigation } from "../navigation/stack-context";
import type { Provider, ProviderInfo, RegionInfo, UserPreferences } from "../types/ipc";
import { ProvisioningView } from "./ProvisioningView";
import "./DisconnectedView.css";

// ── Component ──────────────────────────────────────────────────────────────

/**
 * DisconnectedView -- the initial view shown when no VPN session is active.
 *
 * Responsibilities:
 *   - Fetch available providers on mount (list_providers IPC)
 *   - Auto-select the provider when exactly one is registered
 *   - Fetch regions when a provider is selected (list_regions IPC)
 *   - Attempt last_provider / last_region pre-selection via get_preferences
 *     (silently ignored -- the command is a NOT_IMPLEMENTED stub in M4)
 *   - Allow the user to pick a provider (ProviderSelector, hidden for ≤1)
 *   - Allow the user to pick a region (RegionList with skeleton loading)
 *   - Trigger connect (connect IPC) and push a placeholder Connecting view
 *   - Surface inline errors for each IPC stage with a Retry action
 */
export function DisconnectedView() {
	const { push } = useNavigation();

	// ── State ──────────────────────────────────────────────────────────────

	const [providers, setProviders] = useState<ProviderInfo[]>([]);
	const [selectedProvider, setSelectedProvider] = useState<Provider | null>(null);
	const [regions, setRegions] = useState<RegionInfo[]>([]);
	const [selectedRegion, setSelectedRegion] = useState<string | null>(null);
	const [isLoadingProviders, setIsLoadingProviders] = useState(true);
	const [isLoadingRegions, setIsLoadingRegions] = useState(false);
	const [providerError, setProviderError] = useState<string | null>(null);
	const [regionError, setRegionError] = useState<string | null>(null);

	// ── IPC: list_providers ────────────────────────────────────────────────

	const fetchProviders = useCallback(async () => {
		setIsLoadingProviders(true);
		setProviderError(null);

		try {
			const result = await invoke<ProviderInfo[]>("list_providers");
			setProviders(result);

				// Auto-select when exactly one valid provider is registered
			const validProviders = result.filter((p) => p.status === "valid");
			if (validProviders.length === 1) {
				setSelectedProvider(validProviders[0].provider);
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			setProviderError(message);
		} finally {
			setIsLoadingProviders(false);
		}
	}, []);

	// ── IPC: list_regions ──────────────────────────────────────────────────

	const fetchRegions = useCallback(async (provider: Provider) => {
		setIsLoadingRegions(true);
		setRegionError(null);
		setRegions([]);
		setSelectedRegion(null);

		try {
			const result = await invoke<RegionInfo[]>("list_regions", { provider });
			setRegions(result);
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			setRegionError(message);
		} finally {
			setIsLoadingRegions(false);
		}
	}, []);

	// ── IPC: get_preferences (silent -- NOT_IMPLEMENTED stub) ─────────────

	const applyPreferences = useCallback(
		async (availableProviders: ProviderInfo[]) => {
			try {
				const prefs = await invoke<UserPreferences>(
					"get_preferences",
				);
				if (prefs.lastProvider) {
					const match = availableProviders.find(
						(p) => p.provider === prefs.lastProvider && p.status === "valid",
					);
					if (match) {
						setSelectedProvider(match.provider);
						if (prefs.lastRegion) {
							// Pre-select region after regions load
							setSelectedRegion(prefs.lastRegion);
						}
					}
				}
			} catch {
				// Silently ignored -- get_preferences is a stub in M4
			}
		},
		[],
	);

	// ── Navigate to ProvisioningView ───────────────────────────────────────

	const handleConnect = useCallback(() => {
		if (!selectedProvider || !selectedRegion) return;

		// Build region label from the selected region's display name + provider
		const regionInfo = regions.find((r) => r.region === selectedRegion);
		const regionDisplayName = regionInfo?.displayName ?? selectedRegion;
		const providerLabel =
			selectedProvider.charAt(0).toUpperCase() + selectedProvider.slice(1);
		const regionLabel = `${regionDisplayName} · ${providerLabel}`;

		push(
			"provisioning",
			"Provisioning",
			<ProvisioningView
				provider={selectedProvider}
				regionLabel={regionLabel}
				regionCode={selectedRegion}
			/>,
		);
	}, [selectedProvider, selectedRegion, regions, push]);

	// ── Provider selection handler ─────────────────────────────────────────

	const handleSelectProvider = useCallback(
		(provider: Provider) => {
			if (provider === selectedProvider) return;
			setSelectedProvider(provider);
		},
		[selectedProvider],
	);

	// ── Region selection handler ───────────────────────────────────────────

	const handleSelectRegion = useCallback((region: string) => {
		setSelectedRegion(region);
	}, []);

	// ── Effects ────────────────────────────────────────────────────────────

	// Fetch providers on mount; then attempt preference pre-selection
	useEffect(() => {
		void (async () => {
			setIsLoadingProviders(true);
			setProviderError(null);

			try {
				const result = await invoke<ProviderInfo[]>("list_providers");
				setProviders(result);

				const validProviders = result.filter((p) => p.status === "valid");

				// Auto-select when exactly one provider is registered
				if (validProviders.length === 1) {
					setSelectedProvider(validProviders[0].provider);
				}

				// Attempt preference pre-selection (fails silently)
				await applyPreferences(result);
			} catch (err) {
				const message = err instanceof Error ? err.message : String(err);
				setProviderError(message);
			} finally {
				setIsLoadingProviders(false);
			}
		})();
	}, [applyPreferences]);

	// Fetch regions whenever selectedProvider changes
	useEffect(() => {
		if (!selectedProvider) return;
		void fetchRegions(selectedProvider);
	}, [selectedProvider, fetchRegions]);

	// ── Derived state ──────────────────────────────────────────────────────

	const isConnectDisabled = !selectedProvider || !selectedRegion;

	const showProviderSelector = providers.length > 1;

	// ── Render: provider error ─────────────────────────────────────────────

	if (providerError) {
		return (
			<div className="disconnected-view disconnected-view--error">
				<p className="disconnected-error__message" role="alert">
					Could not load providers: {providerError}
				</p>
				<GlassButton
					variant="neutral"
					onClick={() => void fetchProviders()}
				>
					Retry
				</GlassButton>
			</div>
		);
	}

	// ── Render: loading providers (no skeleton component, brief flash) ─────

	if (isLoadingProviders) {
		return (
			<div className="disconnected-view">
				<RegionList
					regions={[]}
					selectedRegion={null}
					onSelect={handleSelectRegion}
					isLoading={true}
				/>
			</div>
		);
	}

	// ── Render: no providers registered ───────────────────────────────────

	if (providers.length === 0) {
		return (
			<div className="disconnected-view disconnected-view--empty">
				<p className="disconnected-empty__message">
					No cloud providers configured. Add credentials to get started.
				</p>
			</div>
		);
	}

	// ── Render: main ──────────────────────────────────────────────────────

	return (
		<div className="disconnected-view">
			{/* Provider selector -- hidden when ≤1 provider */}
			{showProviderSelector && selectedProvider && (
				<ProviderSelector
					providers={providers}
					selectedProvider={selectedProvider}
					onSelect={handleSelectProvider}
				/>
			)}

			{/* Region list -- skeleton while loading, error fallback otherwise */}
			{regionError ? (
				<div className="disconnected-region-error">
					<p className="disconnected-error__message" role="alert">
						Could not load regions: {regionError}
					</p>
					<GlassButton
						variant="neutral"
						onClick={() =>
							selectedProvider ? void fetchRegions(selectedProvider) : undefined
						}
					>
						Retry
					</GlassButton>
				</div>
			) : (
				<RegionList
					regions={regions}
					selectedRegion={selectedRegion}
					onSelect={handleSelectRegion}
					isLoading={isLoadingRegions}
				/>
			)}

			{/* Connect button */}
			<GlassButton
				variant="success"
				onClick={handleConnect}
				disabled={isConnectDisabled}
			>
				Connect
			</GlassButton>
		</div>
	);
}
