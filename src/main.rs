use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use async_trait::async_trait;
use futures::future::join_all;
use gander::{inventory::load_inventory, playbook::load_playbook};
use russh_keys::key::PublicKey;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let inventory = load_inventory("/home/goose/dev/flock/inventory")?;
    println!("{:#?}", inventory);

    let playbook = load_playbook("/home/goose/dev/flock/playbook.toml")?;
    println!("{:#?}", playbook);

    let ssh_config = Arc::new(russh::client::Config {
        connection_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    });

    println!("Connecting to hosts ...");

    let connection_futures = inventory.hosts.iter().map(|host| {
        let ssh_config = Arc::clone(&ssh_config);
        async move {
            let conn_future = russh::client::connect(
                ssh_config,
                (host.address.as_str(), host.ssh_port),
                Handler {},
            );
            let connection_result = match timeout(Duration::from_secs(5), conn_future).await {
                Ok(Ok(x)) => Ok(x),
                Ok(Err(err)) => Err(anyhow!(err)),
                Err(err) => Err(anyhow!(err)),
            };

            (host, connection_result)
        }
    });

    let connections = join_all(connection_futures).await;

    for (host, connection_result) in &connections {
        println!("{:?} : {:?}", host.path, connection_result.is_ok());
    }

    Ok(())
}

struct Handler {}

#[async_trait]
impl russh::client::Handler for Handler {
    type Error = anyhow::Error;

    async fn check_server_key(
        self,
        server_public_key: &PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        //TODO
        Ok((self, true))
    }
}
