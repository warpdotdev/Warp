//! Tests for the Hermes Agent harness integration.

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_hermes_harness_returns_correct_harness_variant() {
        let harness = HermesHarness;
        assert_eq!(harness.harness(), Harness::Hermes);
    }

    #[test]
    fn test_hermes_harness_returns_correct_cli_agent() {
        let harness = HermesHarness;
        assert_eq!(harness.cli_agent(), CLIAgent::Hermes);
    }

    #[test]
    fn test_hermes_harness_has_install_docs_url() {
        let harness = HermesHarness;
        assert!(harness.install_docs_url().is_some());
        assert!(harness.install_docs_url().unwrap().contains("hermes-agent"));
    }

    #[test]
    fn test_hermes_command_basic() {
        let cmd = hermes_command("hermes", "/tmp/prompt.txt", None);
        assert!(cmd.starts_with("hermes chat -q"));
        assert!(cmd.contains("--yolo"));
        assert!(!cmd.contains("-s"));
    }

    #[test]
    fn test_hermes_command_with_system_prompt() {
        let cmd = hermes_command("hermes", "/tmp/prompt.txt", Some("/tmp/sp.txt"));
        assert!(cmd.contains("-s"));
    }

    #[test]
    fn test_harness_kind_hermes() {
        let kind = super::super::super::harness_kind(Harness::Hermes).unwrap();
        assert!(matches!(kind, HarnessKind::ThirdParty(_)));
    }
}
