use serde_json::{Value, json};

use crate::config::ShimConfig;

const LOCAL_USER_UID: &str = "local-shim-user";
const LOCAL_USER_EMAIL: &str = "local@warp-shim";
const LOCAL_WORKSPACE_UID: &str = "local-shim-workspace";

pub(crate) fn get_feature_model_choices(config: &ShimConfig) -> Value {
    json!({
        "data": {
            "user": {
                "__typename": "UserOutput",
                "user": {
                    "workspaces": [
                        {
                            "featureModelChoice": feature_model_choice(config),
                        }
                    ]
                }
            }
        }
    })
}

pub(crate) fn free_available_models(config: &ShimConfig) -> Value {
    json!({
        "data": {
            "freeAvailableModels": {
                "__typename": "FreeAvailableModelsOutput",
                "featureModelChoice": feature_model_choice(config),
                "responseContext": {
                    "serverVersion": "warp-shim-local",
                }
            }
        }
    })
}

pub(crate) fn get_user(config: &ShimConfig) -> Value {
    json!({
        "data": {
            "user": {
                "__typename": "UserOutput",
                "apiKeyOwnerType": null,
                "principalType": "USER",
                "user": {
                    "anonymousUserInfo": null,
                    "experiments": [],
                    "isOnWorkDomain": false,
                    "isOnboarded": true,
                    "profile": {
                        "displayName": "Warp Shim User",
                        "email": LOCAL_USER_EMAIL,
                        "needsSsoLink": false,
                        "photoUrl": null,
                        "uid": LOCAL_USER_UID,
                    },
                    "llms": feature_model_choice(config),
                }
            }
        }
    })
}

pub(crate) fn get_user_settings() -> Value {
    json!({
        "data": {
            "user": {
                "__typename": "UserOutput",
                "user": {
                    "settings": {
                        "isCloudConversationStorageEnabled": false,
                        "isCrashReportingEnabled": false,
                        "isTelemetryEnabled": false,
                    }
                }
            }
        }
    })
}

pub(crate) fn get_workspaces_metadata_for_user(config: &ShimConfig) -> Value {
    json!({
        "data": {
            "user": {
                "__typename": "UserOutput",
                "user": {
                    "workspaces": [workspace(config)],
                    "experiments": [],
                    "discoverableTeams": [],
                }
            },
            "pricingInfo": {
                "__typename": "PricingInfoOutput",
                "pricingInfo": {
                    "plans": [],
                    "overages": {
                        "pricePerRequestUsdCents": 0,
                    },
                    "addonCreditsOptions": [],
                }
            }
        }
    })
}

pub(crate) fn unknown_operation() -> Value {
    json!({ "data": null })
}

fn feature_model_choice(config: &ShimConfig) -> Value {
    let choices = model_catalog(config);
    json!({
        "agentMode": available_llms(default_model_id(config, "auto"), choices.clone(), None),
        "planning": available_llms(default_model_id(config, "auto"), choices.clone(), None),
        "coding": available_llms(
            default_model_id(config, "coding-auto"),
            choices.clone(),
            preferred_model_id(config, "coding-auto"),
        ),
        "cliAgent": available_llms(
            default_model_id(config, "cli-agent-auto"),
            choices.clone(),
            preferred_model_id(config, "cli-agent-auto"),
        ),
        "computerUseAgent": available_llms(
            default_model_id(config, "computer-use-agent-auto"),
            choices,
            preferred_model_id(config, "computer-use-agent-auto"),
        ),
    })
}

fn available_llms(
    default_id: String,
    choices: Vec<Value>,
    preferred_codex_model_id: Option<String>,
) -> Value {
    json!({
        "defaultId": default_id,
        "choices": choices,
        "preferredCodexModelId": preferred_codex_model_id,
    })
}

fn model_catalog(config: &ShimConfig) -> Vec<Value> {
    let mut models = config
        .models
        .keys()
        .map(|model_id| model_info(model_id))
        .collect::<Vec<_>>();
    if models.is_empty() {
        tracing::warn!("shim config has no model mappings; returning fallback `auto` model");
        models.push(model_info("auto"));
    }
    models
}

fn model_info(model_id: &str) -> Value {
    let display_name = format!("Local {model_id}");
    json!({
        "id": model_id,
        "displayName": display_name,
        "baseModelName": display_name,
        "reasoningLevel": null,
        "usageMetadata": {
            "creditMultiplier": null,
            "requestMultiplier": 1,
        },
        "description": "OpenAI-compatible via warp-shim",
        "disableReason": null,
        "visionSupported": false,
        "spec": {
            "cost": 0,
            "quality": 0,
            "speed": 0,
        },
        "provider": "UNKNOWN",
        "hostConfigs": [
            {
                "enabled": true,
                "modelRoutingHost": "DIRECT_API",
            }
        ],
        "pricing": {
            "discountPercentage": null,
        },
    })
}

fn default_model_id(config: &ShimConfig, preferred: &str) -> String {
    preferred_model_id(config, preferred)
        .or_else(|| preferred_model_id(config, "auto"))
        .unwrap_or_else(|| {
            config
                .models
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "auto".to_string())
        })
}

fn preferred_model_id(config: &ShimConfig, preferred: &str) -> Option<String> {
    config
        .models
        .contains_key(preferred)
        .then(|| preferred.to_string())
}

fn workspace(config: &ShimConfig) -> Value {
    json!({
        "uid": LOCAL_WORKSPACE_UID,
        "name": "Warp Shim Local Workspace",
        "stripeCustomerId": null,
        "members": [],
        "teams": [],
        "billingMetadata": billing_metadata(),
        "bonusGrantsInfo": {
            "grants": [],
            "spendingInfo": null,
        },
        "settings": workspace_settings(),
        "hasBillingHistory": false,
        "inviteCode": null,
        "pendingEmailInvites": [],
        "inviteLinkDomainRestrictions": [],
        "isEligibleForDiscovery": false,
        "featureModelChoice": feature_model_choice(config),
        "totalRequestsUsedSinceLastRefresh": 0,
    })
}

fn billing_metadata() -> Value {
    json!({
        "customerType": "FREE",
        "delinquencyStatus": "NO_DELINQUENCY",
        "tier": {
            "name": "Local Shim",
            "description": "Local shim workspace",
            "warpAiPolicy": {
                "limit": 0,
                "isCodeSuggestionsToggleable": false,
                "isPromptSuggestionsToggleable": false,
                "isNextCommandEnabled": false,
                "isVoiceEnabled": false,
            },
            "teamSizePolicy": {
                "isUnlimited": true,
                "limit": 1,
            },
            "sharedNotebooksPolicy": {
                "isUnlimited": true,
                "limit": 0,
            },
            "sharedWorkflowsPolicy": {
                "isUnlimited": true,
                "limit": 0,
            },
            "sessionSharingPolicy": {
                "enabled": false,
                "maxSessionBytesSize": 0,
            },
            "aiAutonomyPolicy": null,
            "telemetryDataCollectionPolicy": {
                "default": false,
                "toggleable": true,
            },
            "ugcDataCollectionPolicy": {
                "defaultSetting": "DISABLE",
                "toggleable": true,
            },
            "usageBasedPricingPolicy": {
                "toggleable": false,
            },
            "codebaseContextPolicy": {
                "toggleable": false,
                "isUnlimitedIndices": true,
                "maxIndices": 0,
                "maxFilesPerRepo": 0,
                "embeddingGenerationBatchSize": 0,
            },
            "byoApiKeyPolicy": {
                "enabled": true,
            },
            "purchaseAddOnCreditsPolicy": {
                "enabled": false,
            },
            "enterprisePayAsYouGoPolicy": null,
            "enterpriseCreditsAutoReloadPolicy": null,
            "multiAdminPolicy": {
                "enabled": false,
            },
            "ambientAgentsPolicy": {
                "enabled": false,
                "toggleable": false,
                "maxConcurrentAgents": 0,
                "instanceShape": null,
            },
        },
        "serviceAgreements": [],
        "aiOverages": null,
    })
}

fn workspace_settings() -> Value {
    json!({
        "isDiscoverable": false,
        "isInviteLinkEnabled": false,
        "llmSettings": {
            "enabled": true,
            "hostConfigs": [
                {
                    "host": "DIRECT_API",
                    "settings": {
                        "enabled": true,
                        "optOutOfNewModels": false,
                        "enablementSetting": "RESPECT_USER_SETTING",
                    }
                }
            ],
        },
        "telemetrySettings": {
            "forceEnabled": false,
        },
        "ugcCollectionSettings": {
            "setting": "DISABLE",
        },
        "cloudConversationStorageSettings": {
            "setting": "DISABLE",
        },
        "aiPermissionsSettings": {
            "allowAiInRemoteSessions": false,
            "remoteSessionRegexList": [],
        },
        "linkSharingSettings": {
            "anyoneWithLinkSharingEnabled": false,
            "directLinkSharingEnabled": false,
        },
        "secretRedactionSettings": {
            "enabled": false,
            "regexes": [],
        },
        "aiAutonomySettings": {
            "applyCodeDiffsSetting": null,
            "readFilesSetting": null,
            "readFilesAllowlist": null,
            "createPlansSetting": null,
            "executeCommandsSetting": null,
            "executeCommandsAllowlist": null,
            "executeCommandsDenylist": null,
            "writeToPtySetting": null,
            "computerUseSetting": null,
        },
        "usageBasedPricingSettings": {
            "enabled": false,
            "maxMonthlySpendCents": null,
        },
        "addonCreditsSettings": {
            "autoReloadEnabled": false,
            "maxMonthlySpendCents": null,
            "selectedAutoReloadCreditDenomination": null,
        },
        "codebaseContextSettings": {
            "enabled": false,
            "setting": "DISABLE",
        },
        "sandboxedAgentSettings": null,
    })
}

#[cfg(test)]
#[path = "graphql_payloads_tests.rs"]
mod tests;
