import "./ProvisioningStepper.css";

// ── Types ──────────────────────────────────────────────────────────────────

type StepState = "done" | "active" | "waiting" | "failed";

type ProvisioningStepperProps = {
	/**
	 * Current active step (1--3). Steps below this are "done", this step
	 * is "active", steps above are "waiting". 0 means all waiting.
	 */
	currentStep: number;
	/** Step number that failed (1--3), or null if no failure. */
	failedStep: number | null;
};

// ── Constants ──────────────────────────────────────────────────────────────

const STEPS = [
	{ number: 1, title: "Creating server" },
	{ number: 2, title: "Installing WireGuard" },
	{ number: 3, title: "Connecting tunnel" },
] as const;

// ── Helpers ────────────────────────────────────────────────────────────────

function resolveStepState(
	stepNumber: number,
	currentStep: number,
	failedStep: number | null,
): StepState {
	if (failedStep === stepNumber) return "failed";
	if (stepNumber < currentStep) return "done";
	if (stepNumber === currentStep && failedStep === null) return "active";
	return "waiting";
}

function stepDescription(state: StepState): string | null {
	switch (state) {
		case "done":
			return "Completed";
		case "active":
			return "In progress...";
		case "failed":
			return "Failed";
		case "waiting":
			return null;
	}
}

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Vertical 3-step stepper for the provisioning flow.
 *
 * Pure display component -- receives step state as props, no IPC knowledge.
 * Follows the Liquid Glass design language with tinted step indicators.
 *
 * Step states:
 *   done    → ✓ green circle, "Completed"
 *   active  → spinner amber circle, "In progress..."
 *   waiting → number gray circle, no description
 *   failed  → ✕ red circle, error message inline
 */
export function ProvisioningStepper({
	currentStep,
	failedStep,
}: ProvisioningStepperProps) {
	return (
		<ol className="stepper" aria-label="Provisioning progress">
			{STEPS.map((step, index) => {
				const state = resolveStepState(step.number, currentStep, failedStep);
				const description = stepDescription(state);
				const isLast = index === STEPS.length - 1;

				return (
					<li
						key={step.number}
						className={`stepper-step stepper-step--${state}`}
						aria-current={state === "active" ? "step" : undefined}
					>
						{/* Circle indicator */}
						<div className="stepper-indicator">
							<div className={`stepper-circle stepper-circle--${state}`}>
								{state === "done" && <span className="stepper-icon">✓</span>}
								{state === "active" && <span className="stepper-spinner" />}
								{state === "waiting" && (
									<span className="stepper-number">{step.number}</span>
								)}
								{state === "failed" && <span className="stepper-icon">✕</span>}
							</div>
							{/* Connector line (not on last step) */}
							{!isLast && <div className={`stepper-connector stepper-connector--${state}`} />}
						</div>

						{/* Text content */}
						<div className="stepper-content">
							<span className={`stepper-title stepper-title--${state}`}>{step.title}</span>
							{description && (
								<span className={`stepper-description stepper-description--${state}`}>
									{description}
								</span>
							)}
						</div>
					</li>
				);
			})}
		</ol>
	);
}
