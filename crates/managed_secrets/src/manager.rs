use std::{collections::HashMap, future::Future, sync::Arc};

use warpui::{Entity, SingletonEntity};

use crate::{
    ManagedSecret, ManagedSecretValue,
    client::{
        IdentityTokenOptions, ManagedSecretConfigs, ManagedSecretsClient, SecretOwner,
        TaskIdentityToken,
    },
    envelope::UploadKey,
};

/// Singleton model for working with Warp-managed secrets.
pub struct ManagedSecretManager {
    client: Arc<dyn ManagedSecretsClient>,
    actor_provider: Arc<dyn ActorProvider>,
}

pub trait ActorProvider: Send + Sync + 'static {
    fn actor_uid(&self) -> Option<String>;
}

impl ManagedSecretManager {
    pub fn new(
        client: Arc<dyn ManagedSecretsClient>,
        actor_provider: Arc<dyn ActorProvider>,
    ) -> Self {
        crate::envelope::init();
        Self {
            client,
            actor_provider,
        }
    }

    pub fn create_secret(
        &self,
        owner: SecretOwner,
        name: String,
        value: ManagedSecretValue,
        description: Option<String>,
    ) -> impl Future<Output = anyhow::Result<ManagedSecret>> + use<> {
        let client = self.client.clone();
        let actor_provider = self.actor_provider.clone();
        async move {
            // We retrieve all upload keys on demand. These should potentially be fetched and stored
            // ahead of time instead.
            let configs = client.get_managed_secret_configs().await?;

            let Some(actor) = actor_provider.actor_uid() else {
                return Err(anyhow::anyhow!("No authenticated user"));
            };

            // Chain errors so that we don't hold an `UploadKey` handle across an `.await`.
            let encrypted_value = owner_public_key(&configs, &owner)
                .and_then(|public_key| {
                    UploadKey::import_public_keyset(public_key).map_err(anyhow::Error::from)
                })
                .and_then(|public_key| {
                    public_key
                        .encrypt_secret(&actor, &name, &value)
                        .map_err(anyhow::Error::from)
                })?;

            let managed_secret = client
                .create_managed_secret(
                    owner,
                    name,
                    value.secret_type(),
                    encrypted_value,
                    description,
                )
                .await?;
            Ok(managed_secret)
        }
    }

    pub fn delete_secret(
        &self,
        owner: SecretOwner,
        name: String,
    ) -> impl Future<Output = anyhow::Result<()>> + use<> {
        let client = self.client.clone();
        async move {
            client.delete_managed_secret(owner, name).await?;
            Ok(())
        }
    }

    pub fn update_secret(
        &self,
        owner: SecretOwner,
        name: String,
        value: Option<ManagedSecretValue>,
        description: Option<String>,
    ) -> impl Future<Output = anyhow::Result<ManagedSecret>> + use<> {
        let client = self.client.clone();
        let actor_provider = self.actor_provider.clone();
        async move {
            let encrypted_value = if let Some(value) = value {
                // We retrieve all upload keys on demand. These should potentially be fetched and stored
                // ahead of time instead.
                let configs = client.get_managed_secret_configs().await?;

                let Some(actor) = actor_provider.actor_uid() else {
                    return Err(anyhow::anyhow!("No authenticated user"));
                };

                // Chain errors so that we don't hold an `UploadKey` handle across an `.await`.
                let encrypted = owner_public_key(&configs, &owner)
                    .and_then(|public_key| {
                        UploadKey::import_public_keyset(public_key).map_err(anyhow::Error::from)
                    })
                    .and_then(|public_key| {
                        public_key
                            .encrypt_secret(&actor, &name, &value)
                            .map_err(anyhow::Error::from)
                    })?;
                Some(encrypted)
            } else {
                None
            };

            let managed_secret = client
                .update_managed_secret(owner, name, encrypted_value, description)
                .await?;
            Ok(managed_secret)
        }
    }

    /// List all managed secrets accessible to the current user.
    pub fn list_secrets(&self) -> impl Future<Output = anyhow::Result<Vec<ManagedSecret>>> + use<> {
        let client = self.client.clone();
        async move {
            let secrets = client.list_secrets().await?;
            Ok(secrets)
        }
    }

    /// Get Warp-managed secrets scoped to the currently-executing task.
    ///
    /// This will fail if not in an ambient agent.
    pub fn get_task_secrets(
        &self,
        task_id: String,
    ) -> impl Future<Output = anyhow::Result<HashMap<String, ManagedSecretValue>>> + use<> {
        let client = self.client.clone();
        async move {
            let secrets = client.get_task_secrets(task_id).await?;
            Ok(secrets)
        }
    }

    /// Issue a short-lived OIDC identity token for the current task.
    pub fn issue_task_identity_token(
        &self,
        options: IdentityTokenOptions,
    ) -> impl Future<Output = anyhow::Result<TaskIdentityToken>> + use<> {
        let client = self.client.clone();
        async move { client.issue_task_identity_token(options).await }
    }
}

/// Find the public upload key corresponding to `owner`.
/// Returns an error if there's no such key in `configs`.
fn owner_public_key<'a>(
    configs: &'a ManagedSecretConfigs,
    owner: &SecretOwner,
) -> Result<&'a str, anyhow::Error> {
    match owner {
        SecretOwner::CurrentUser => configs
            .user_secrets
            .as_ref()
            .and_then(|config| config.public_key.as_deref())
            .ok_or_else(|| anyhow::anyhow!("No public key for user")),
        SecretOwner::Team { team_uid } => configs
            .team_secrets
            .get(team_uid)
            .and_then(|config| config.public_key.as_deref())
            .ok_or_else(|| anyhow::anyhow!("No public key for team {team_uid}")),
    }
}

impl Entity for ManagedSecretManager {
    type Event = ();
}

impl SingletonEntity for ManagedSecretManager {}
