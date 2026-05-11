# Environment Setup Failure UI for Cloud Mode (V2)

## Summary
When a cloud agent's environment setup fails (clone, setup command, or cd), the cloud mode V2 UI gets stuck showing "Setting up environment" indefinitely. The setup failure should surface clearly: the failed command is auto-expanded with error styling, and the error footer above the input shows the server-reported error message.

Figma: none provided

## Behavior

### Detection and error display

1. When any environment setup command (git clone, setup command, or cd) exits with a non-zero exit code during the cloud mode V2 setup phase, the UI must transition out of the "setting up environment" state within a bounded time.

2. After detecting a setup command failure, the client reads the task's error message from `AgentConversationsModel` (the same data source the conversation details side panel uses). If the server-reported error is already available (e.g. via RTC push), it is used immediately. If not, a fetch is triggered and the error footer appears once the data arrives.

3. Once the server error message is available, an error footer renders above the input area. The footer has a red-tinted background and border (matching the existing `render_error_footer` styling) with:
   - Header text: "Environment setup failed"
   - Body text: the server-reported error message from the task's `status_message`

4. If the task data fetch fails or the `status_message` is absent, the footer falls back to a generic body: "Environment setup failed. Check your environment's repository URLs and setup commands."

5. While waiting for the server error (between detecting the failure and the task data arriving), the "Setting up environment" loading footer remains visible. The user should not see a blank or flickering state during this window.

### Setup command text and expansion

6. When a setup command fails, the collapsible "Running setup commands…" text changes to "Setup failed" with error styling (red/error color treatment matching the theme's error color).

7. When a setup command fails, the setup command group auto-expands so the failed command block (which already shows a red X icon) is visible to the user without manual interaction.

8. If the group was manually collapsed by the user before the failure, the auto-expand on failure overrides that preference.

### Input and recovery

9. After the error footer is displayed, no interactive input is shown. The user cannot type a new prompt or retry from the same pane.

10. The user dismisses the failed pane by closing it (same as any other terminal tab) and starts a new cloud mode session if they want to retry.

### Invariants

11. The "Setting up environment" loading footer must never persist indefinitely. Any terminal failure state during environment setup must eventually resolve to the error footer or a session disconnect.

12. The error footer must show the same error message that the conversation details side panel shows for the same failure. Both derive from the task's server-reported `status_message`.

13. The individual setup command block's red X icon behavior is unchanged — it continues to show success/failure per-command as it does today.

14. Setup command failures that occur before the shared session connects (i.e., the model is still in `WaitingForSession`) continue to use the existing pre-V2 error handling path. This spec only covers failures that occur after the session has connected and the V2 inline setup UI is active.

15. Follow-up runs (cloud-to-cloud follow-ups) that fail during environment setup exhibit the same behavior: the setup text changes to "Setup failed", the group auto-expands, and the error footer appears with the server message.
