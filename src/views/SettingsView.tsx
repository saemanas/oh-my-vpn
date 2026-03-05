import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { GlassButton } from "../components/GlassButton";
import { GlassInput } from "../components/GlassInput";
import type { Provider, UserPreferences } from "../types/ipc";
import "./SettingsView.css";

// ── Local types ────────────────────────────────────────────────────────────

/**
 * Partial update payload for update_preferences IPC command.
 * Omitted fields are not modified on the backend.
 * A null value for optional fields (e.g. keyboardShortcut) clears the field.
 */
interface PartialUserPreferences {
	lastProvider?: Provider | null;
	lastRegion?: string | null;
	notificationsEnabled?: boolean;
	keyboardShortcut?: string | null;
}

/** Transient save feedback status. */
type SaveStatus = "idle" | "saving" | "success" | "error";

// ── Component ──────────────────────────────────────────────────────────────

/**
 * SettingsView -- user preferences panel.
 *
 * Responsibilities:
 *   - Load current preferences on mount (get_preferences IPC)
 *   - Toggle notification preference immediately on switch interaction
 *   - Persist keyboard shortcut on input blur
 *   - Surface inline save feedback (success / error flash)
 *   - Show loading skeleton and error-with-retry states
 */
export function SettingsView() {
	// ── State ──────────────────────────────────────────────────────────────

	const [preferences, setPreferences] = useState<UserPreferences | null>(null);
	const [isLoading, setIsLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);

	/** Controlled value for the keyboard shortcut input. */
	const [shortcutValue, setShortcutValue] = useState("");

	/** Transient save feedback -- auto-resets after a brief delay. */
	const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
	const [saveError, setSaveError] = useState<string | null>(null);
	const feedbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

	// ── Feedback helpers ───────────────────────────────────────────────────

	/**
	 * Show transient feedback then reset to idle after 2 s.
	 * Clears any in-flight timer to avoid double-reset.
	 */
	const showFeedback = useCallback(
		(status: "success" | "error", message?: string) => {
			if (feedbackTimerRef.current !== null) {
				clearTimeout(feedbackTimerRef.current);
			}
			setSaveStatus(status);
			setSaveError(message ?? null);
			feedbackTimerRef.current = setTimeout(() => {
				setSaveStatus("idle");
				setSaveError(null);
				feedbackTimerRef.current = null;
			}, 2000);
		},
		[],
	);

	// ── IPC: get_preferences ───────────────────────────────────────────────

	const loadPreferences = useCallback(async () => {
		setIsLoading(true);
		setError(null);

		try {
			const result = await invoke<UserPreferences>("get_preferences");
			setPreferences(result);
			setShortcutValue(result.keyboardShortcut ?? "");
		} catch (err) {
			const message = err instanceof Error ? err.message : String(err);
			setError(message);
		} finally {
			setIsLoading(false);
		}
	}, []);

	// ── IPC: update_preferences ────────────────────────────────────────────

	const savePreferences = useCallback(
		async (updates: PartialUserPreferences) => {
			setSaveStatus("saving");
			try {
				const result = await invoke<UserPreferences>("update_preferences", {
					preferences: updates,
				});
				setPreferences(result);
				setShortcutValue(result.keyboardShortcut ?? "");
				showFeedback("success");
			} catch (err) {
				const message = err instanceof Error ? err.message : String(err);
				showFeedback("error", message);
			}
		},
		[showFeedback],
	);

	// ── Handlers ───────────────────────────────────────────────────────────

	const handleToggleNotifications = useCallback(() => {
		if (!preferences) return;
		void savePreferences({
			notificationsEnabled: !preferences.notificationsEnabled,
		});
	}, [preferences, savePreferences]);

	const handleShortcutBlur = useCallback(() => {
		if (!preferences) return;
		const trimmed = shortcutValue.trim();
		const current = preferences.keyboardShortcut ?? "";
		// Only persist if the value actually changed.
		if (trimmed === current) return;
		void savePreferences({
			keyboardShortcut: trimmed.length > 0 ? trimmed : null,
		});
	}, [preferences, shortcutValue, savePreferences]);

	// ── Effects ────────────────────────────────────────────────────────────

	useEffect(() => {
		void loadPreferences();
		return () => {
			// Cleanup feedback timer on unmount.
			if (feedbackTimerRef.current !== null) {
				clearTimeout(feedbackTimerRef.current);
			}
		};
	}, [loadPreferences]);

	// ── Render: error ──────────────────────────────────────────────────────

	if (error && !isLoading) {
		return (
			<div className="settings-view settings-view--error">
				<p className="settings-error__message" role="alert">
					Could not load preferences: {error}
				</p>
				<GlassButton variant="neutral" onClick={() => void loadPreferences()}>
					Retry
				</GlassButton>
			</div>
		);
	}

	// ── Render: loading ────────────────────────────────────────────────────

	if (isLoading) {
		return (
			<div className="settings-view settings-view--loading">
				<div className="settings-skeleton">
					<div className="settings-skeleton__row" />
					<div className="settings-skeleton__row" />
				</div>
			</div>
		);
	}

	// ── Render: main ───────────────────────────────────────────────────────

	const isSaving = saveStatus === "saving";

	return (
		<div className="settings-view">
			{/* Notifications row */}
			<div className="settings-section">
				<div className="liquidGlass-wrapper settings-row">
					<div className="liquidGlass-effect" />
					<div className="liquidGlass-tint" />
					<div className="liquidGlass-shine" />
					<div className="liquidGlass-text settings-row__content">
						<div className="settings-row__label-group">
							<label
								className="settings-row__label"
								htmlFor="notifications-toggle"
							>
								Notifications
							</label>
							<span className="settings-row__description">
								Show alerts when the VPN connects or disconnects
							</span>
						</div>
						<div className="settings-toggle-wrapper">
							<input
								id="notifications-toggle"
								type="checkbox"
								role="switch"
								className="settings-toggle__input"
								checked={preferences?.notificationsEnabled ?? false}
								aria-checked={preferences?.notificationsEnabled ?? false}
								onChange={handleToggleNotifications}
								disabled={isSaving}
								aria-label="Enable notifications"
							/>
							<span className="settings-toggle__track" aria-hidden="true">
								<span className="settings-toggle__thumb" />
							</span>
						</div>
					</div>
				</div>
			</div>

			{/* Keyboard shortcut row */}
			<div className="settings-section">
				<label
					className="settings-section__label"
					htmlFor="keyboard-shortcut-input"
				>
					Keyboard Shortcut
				</label>
				<span className="settings-section__description">
					Global hotkey to toggle the VPN (e.g. Ctrl+Alt+V). Leave empty to
					disable.
				</span>
				<GlassInput
					value={shortcutValue}
					onChange={setShortcutValue}
					onBlur={handleShortcutBlur}
					placeholder="e.g. Ctrl+Alt+V"
					disabled={isSaving}
					className="settings-shortcut-input"
				/>
			</div>

			{/* Save feedback */}
			{saveStatus !== "idle" && (
				<output
					className={`settings-feedback settings-feedback--${saveStatus}`}
					aria-live="polite"
				>
					{saveStatus === "saving" && (
						<span className="settings-feedback__text">Saving…</span>
					)}
					{saveStatus === "success" && (
						<span className="settings-feedback__text">✓ Saved</span>
					)}
					{saveStatus === "error" && (
						<span className="settings-feedback__text">
							⚠ {saveError ?? "Failed to save"}
						</span>
					)}
				</output>
			)}
		</div>
	);
}
