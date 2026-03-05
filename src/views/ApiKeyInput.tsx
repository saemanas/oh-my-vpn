import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import { GlassButton } from "../components/GlassButton";
import { GlassInput } from "../components/GlassInput";
import { useNavigation } from "../navigation/stack-context";
import type { AppError, Provider, ProviderInfo } from "../types/ipc";
import { SuccessScreen } from "./SuccessScreen";
import "./ApiKeyInput.css";

// ── Provider metadata ──────────────────────────────────────────────────────

type ProviderMeta = {
	labelPlaceholder: string;
	helpUrl: string;
	helpText: string;
};

const PROVIDER_META: Record<Provider, ProviderMeta> = {
	hetzner: {
		labelPlaceholder: "My Hetzner Account",
		helpUrl:
			"https://docs.hetzner.com/cloud/api/getting-started/generating-api-token/",
		helpText: "How to get your Hetzner API key",
	},
	aws: {
		labelPlaceholder: "My AWS Account",
		helpUrl:
			"https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html",
		helpText: "How to get your AWS access key",
	},
	gcp: {
		labelPlaceholder: "My GCP Account",
		helpUrl: "https://cloud.google.com/compute/docs/authentication",
		helpText: "How to get your GCP credentials",
	},
};

// ── Error mapping ──────────────────────────────────────────────────────────

/**
 * Per-field error codes: set as an error prop on the matching GlassInput.
 * General error codes: displayed as a standalone alert below the inputs.
 */
const API_KEY_ERROR_CODES = new Set([
	"AUTH_INVALID_KEY",
	"AUTH_INSUFFICIENT_PERMISSIONS",
	"VALIDATION_EMPTY_API_KEY",
]);

const ACCOUNT_LABEL_ERROR_CODES = new Set(["VALIDATION_EMPTY_ACCOUNT_LABEL"]);

const ERROR_MESSAGES: Record<string, string> = {
	AUTH_INVALID_KEY: "Invalid API key. Please check and try again.",
	AUTH_INSUFFICIENT_PERMISSIONS:
		"Insufficient permissions. See provider setup guide.",
	PROVIDER_TIMEOUT:
		"Could not reach the provider. Check your connection and try again.",
	VALIDATION_EMPTY_API_KEY: "API key cannot be empty.",
	VALIDATION_EMPTY_ACCOUNT_LABEL: "Account label cannot be empty.",
	KEYCHAIN_ACCESS_DENIED: "Failed to save credentials to keychain.",
	KEYCHAIN_WRITE_FAILED: "Failed to save credentials to keychain.",
	INTERNAL_UNEXPECTED: "An unexpected error occurred.",
};

function mapErrorCode(code: string, fallback: string): string {
	return ERROR_MESSAGES[code] ?? fallback;
}

// ── Types ──────────────────────────────────────────────────────────────────

type ApiKeyInputProps = {
	/** Cloud provider selected on the previous screen. */
	provider: Provider;
};

type ErrorState = {
	code: string;
	message: string;
} | null;

// ── Component ──────────────────────────────────────────────────────────────

/**
 * ApiKeyInput -- onboarding step for entering and validating cloud credentials.
 *
 * Renders:
 *   - A password input for the API key
 *   - A text input for a human-readable account label
 *   - A help link that opens provider documentation in the default browser
 *   - A "Validate" button that calls the register_provider IPC command
 *
 * On success: pushes SuccessScreen with the validated ProviderInfo.
 * On error: maps AppError codes to user-friendly messages on the relevant field.
 */
export function ApiKeyInput({ provider }: ApiKeyInputProps) {
	const { push, pop } = useNavigation();
	const meta = PROVIDER_META[provider];

	// ── State ──────────────────────────────────────────────────────────────

	const [apiKey, setApiKey] = useState("");
	const [accountLabel, setAccountLabel] = useState("");
	const [isLoading, setIsLoading] = useState(false);
	const [errorState, setErrorState] = useState<ErrorState>(null);

	// Track unmount to prevent state updates after cleanup
	const unmountedRef = useRef(false);

	useEffect(() => {
		return () => {
			unmountedRef.current = true;
		};
	}, []);

	// ── Derived error routing ──────────────────────────────────────────────

	const errorCode = errorState?.code ?? "";
	const errorMessage = errorState?.message ?? "";

	const apiKeyError = API_KEY_ERROR_CODES.has(errorCode)
		? errorMessage
		: undefined;
	const accountLabelError = ACCOUNT_LABEL_ERROR_CODES.has(errorCode)
		? errorMessage
		: undefined;
	const generalError =
		!API_KEY_ERROR_CODES.has(errorCode) &&
		!ACCOUNT_LABEL_ERROR_CODES.has(errorCode) &&
		errorState !== null
			? errorMessage
			: null;

	// ── Handlers ───────────────────────────────────────────────────────────

	const handleHelpLink = useCallback(() => {
		void openUrl(meta.helpUrl);
	}, [meta.helpUrl]);

	const handleValidate = useCallback(async () => {
		if (unmountedRef.current || isLoading) return;

		setErrorState(null);
		setIsLoading(true);

		try {
			const providerInfo = await invoke<ProviderInfo>("register_provider", {
				provider,
				apiKey,
				accountLabel,
			});

			if (!unmountedRef.current) {
				push(
					"success",
					"Connected",
					<SuccessScreen providerInfo={providerInfo} />,
				);
			}
		} catch (err: unknown) {
			if (!unmountedRef.current) {
				const appError = err as AppError;
				const code = appError.code ?? "";
				const message = mapErrorCode(code, appError.message ?? String(err));
				setErrorState({ code, message });
				setIsLoading(false);
			}
		}
	}, [isLoading, provider, apiKey, accountLabel, push]);

	const handleBack = useCallback(() => {
		pop();
	}, [pop]);

	// ── Render ─────────────────────────────────────────────────────────────

	return (
		<div className="api-key-input">
			{/* Inputs */}
			<div className="api-key-input__fields">
				<GlassInput
					type="password"
					value={apiKey}
					onChange={setApiKey}
					placeholder="Paste your API key"
					disabled={isLoading}
					error={apiKeyError}
				/>
				<GlassInput
					type="text"
					value={accountLabel}
					onChange={setAccountLabel}
					placeholder={meta.labelPlaceholder}
					disabled={isLoading}
					error={accountLabelError}
				/>
			</div>

			{/* Help link */}
			<button
				type="button"
				className="api-key-input__help-link"
				onClick={handleHelpLink}
			>
				{meta.helpText} ↗
			</button>

			{/* General error (network, keychain, internal) */}
			{generalError !== null && (
				<p className="api-key-input__error" role="alert">
					{generalError}
				</p>
			)}

			{/* Actions */}
			<div className="api-key-input__actions">
				<GlassButton
					variant="neutral"
					onClick={handleBack}
					disabled={isLoading}
				>
					Back
				</GlassButton>
				<GlassButton
					variant="success"
					onClick={() => void handleValidate()}
					loading={isLoading}
				>
					Validate
				</GlassButton>
			</div>
		</div>
	);
}
