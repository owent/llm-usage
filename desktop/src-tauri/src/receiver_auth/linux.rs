//! Current-user Secret Service; no collection creation or automatic unlocking.
use super::{Store, SystemStore};
use secret_service::{Collection, EncryptionType, SecretService};
use std::{collections::HashMap, future::Future, time::Duration};

const SERVICE: &str = "org.owent.llm-usage.otel.v1";
const TIMEOUT: Duration = Duration::from_secs(3);

fn attributes(name: &str) -> HashMap<&str, &str> {
    HashMap::from([("application", SERVICE), ("target", name)])
}

async fn persistent_default<'a>(
    service: &'a SecretService<'_>,
) -> Result<Collection<'a>, secret_service::Error> {
    let collection = service.get_default_collection().await?;
    collection.ensure_unlocked().await?;
    // Aliases are configurable. A default alias pointing at the transient
    // session collection must not satisfy a persisted exporter binding.
    match service.get_collection_by_alias("session").await {
        Ok(session) if session.collection_path == collection.collection_path => {
            return Err(secret_service::Error::NoResult);
        }
        Ok(_) | Err(secret_service::Error::NoResult) => {}
        Err(error) => return Err(error),
    }
    Ok(collection)
}

// Store is synchronous and may be called from a Tauri async runtime. Isolate
// the current-thread reactor so nested runtime calls cannot panic. Dropping
// the timed-out future closes its connection; no credential cache survives.
pub(super) fn run<T: Send>(
    future: impl Future<Output = Result<T, secret_service::Error>> + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        scope
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| "credential_store_unavailable".to_string())?;
                runtime.block_on(async {
                    tokio::time::timeout(TIMEOUT, future)
                        .await
                        .map_err(|_| "credential_store_unavailable".to_string())?
                        .map_err(|_| "credential_store_unavailable".to_string())
                })
            })
            .join()
            .unwrap_or_else(|_| Err("credential_store_unavailable".into()))
    })
}

impl Store for SystemStore {
    fn random(&self, bytes: &mut [u8]) -> Result<(), String> {
        super::system_random(bytes)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        run(async {
            let service = SecretService::connect(EncryptionType::Dh).await?;
            let collection = persistent_default(&service).await?;
            // Search all collections to reject ambiguous or locked copies,
            // including ones left by an external writer. Never pick a first match.
            let mut matches = service.search_items(attributes(name)).await?;
            if !matches.locked.is_empty() || matches.unlocked.len() > 1 {
                return Err(secret_service::Error::Locked);
            }
            match matches.unlocked.pop() {
                Some(item) => {
                    let prefix = format!("{}/", collection.collection_path.as_str());
                    if !item.item_path.as_str().starts_with(&prefix) {
                        return Err(secret_service::Error::NoResult);
                    }
                    Ok(Some(item.get_secret().await?))
                }
                None => Ok(None),
            }
        })
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        run(async {
            let service = SecretService::connect(EncryptionType::Dh).await?;
            let collection = persistent_default(&service).await?;
            let matches = service.search_items(attributes(name)).await?;
            if !matches.locked.is_empty() || !matches.unlocked.is_empty() {
                return Err(secret_service::Error::Locked);
            }
            collection
                .create_item(
                    "LLM Usage local telemetry",
                    attributes(name),
                    bytes,
                    false,
                    "application/json",
                )
                .await?;
            Ok(())
        })
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        run(async {
            let service = SecretService::connect(EncryptionType::Dh).await?;
            let mut matches = service.search_items(attributes(name)).await?;
            if !matches.locked.is_empty() || matches.unlocked.len() > 1 {
                return Err(secret_service::Error::Locked);
            }
            if let Some(item) = matches.unlocked.pop() {
                item.delete().await?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stalled_vault_operation_is_bounded_even_inside_an_async_runtime() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let start = std::time::Instant::now();
        runtime.block_on(async {
            assert_eq!(
                super::run::<()>(std::future::pending()).unwrap_err(),
                "credential_store_unavailable"
            );
        });
        assert!(start.elapsed() >= super::TIMEOUT);
        assert!(start.elapsed() < std::time::Duration::from_secs(6));
    }
}
