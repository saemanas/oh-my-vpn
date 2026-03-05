import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback } from "react";
import { GlassButton } from "../components/GlassButton";
import "./SystemPermissionsView.css";

// ── Component ──────────────────────────────────────────────────────────────

/**
 * SystemPermissionsView -- informs users about the sudo requirement.
 *
 * Responsibilities:
 *   - Explain that wg-quick requires root to manage WireGuard tunnels (ADR-0001)
 *   - Clarify that macOS shows an osascript sudo prompt on each connection
 *     because no Network Extension is used in MVP (ADR-0003)
 *   - Provide a direct link to macOS Security & Privacy settings
 *   - Pure presentation -- no IPC calls or async state
 */
export function SystemPermissionsView() {
	// ── Handlers ───────────────────────────────────────────────────────────

	const handleOpenSettings = useCallback(() => {
		void openUrl(
			"x-apple.systempreferences:com.apple.preference.security?Privacy",
		);
	}, []);

	// ── Render ─────────────────────────────────────────────────────────────

	return (
		<div className="system-permissions-view">
			{/* Sudo requirement card */}
			<div className="system-permissions-section">
				<div className="liquidGlass-wrapper system-permissions-card">
					<div className="liquidGlass-effect" />
					<div className="liquidGlass-tint" />
					<div className="liquidGlass-shine" />
					<div className="liquidGlass-text system-permissions-card__content">
						<span className="system-permissions-card__icon" aria-hidden="true">
							🔐
						</span>
						<div className="system-permissions-card__body">
							<h2 className="system-permissions-card__title">
								Sudo Access Required
							</h2>
							<p className="system-permissions-card__description">
								Oh My VPN uses <code className="system-permissions-code">wg-quick</code> to
								create and manage WireGuard tunnels. Because wg-quick must
								create kernel network interfaces, it requires root (administrator)
								privileges.
							</p>
							<p className="system-permissions-card__description">
								Rather than a persistent Network Extension, Oh My VPN invokes{" "}
								<code className="system-permissions-code">osascript</code> to
								request sudo elevation on each connect and disconnect. macOS will
								show its native password prompt at that moment -- this is expected
								behavior.
							</p>
						</div>
					</div>
				</div>
			</div>

			{/* What to expect card */}
			<div className="system-permissions-section">
				<span className="system-permissions-section__label">
					What to Expect
				</span>
				<div className="liquidGlass-wrapper system-permissions-card">
					<div className="liquidGlass-effect" />
					<div className="liquidGlass-tint" />
					<div className="liquidGlass-shine" />
					<div className="liquidGlass-text system-permissions-card__content">
						<ul
							className="system-permissions-list"
							aria-label="Expected sudo behavior"
						>
							<li className="system-permissions-list__item">
								<span
									className="system-permissions-list__bullet"
									aria-hidden="true"
								>
									🔑
								</span>
								<span>
									macOS will show a password prompt when you connect to or
									disconnect from a VPN session.
								</span>
							</li>
							<li className="system-permissions-list__item">
								<span
									className="system-permissions-list__bullet"
									aria-hidden="true"
								>
									✅
								</span>
								<span>
									This is normal --{" "}
									<code className="system-permissions-code">wg-quick</code>{" "}
									needs root access to manage network interfaces.
								</span>
							</li>
							<li className="system-permissions-list__item">
								<span
									className="system-permissions-list__bullet"
									aria-hidden="true"
								>
									🛡️
								</span>
								<span>
									No permanent system changes are made. Elevation is granted only
									for the duration of the tunnel operation and then released.
								</span>
							</li>
						</ul>
					</div>
				</div>
			</div>

			{/* Open System Settings */}
			<div className="system-permissions-section">
				<span className="system-permissions-section__description">
					You can review macOS Privacy &amp; Security settings at any time. No
					additional configuration is required for Oh My VPN to function.
				</span>
				<GlassButton variant="info" onClick={handleOpenSettings}>
					Open System Settings
				</GlassButton>
			</div>
		</div>
	);
}
