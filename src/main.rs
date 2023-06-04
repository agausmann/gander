use std::{fs::File, sync::Arc, time::Duration};

use anyhow::{anyhow, bail, Context as _};
use async_trait::async_trait;
use futures::future::join_all;
use gander::{
    inventory::{load_inventory, HostSpec},
    playbook::load_playbook,
};
use gpgme::{Context, Protocol};
use russh::{client, ChannelId, ChannelMsg};
use russh_keys::{agent::client::AgentClient, decode_openssh, decode_secret_key, key::PublicKey};
use tokio::{
    io::{duplex, DuplexStream},
    sync::mpsc,
    task::JoinHandle,
    time::timeout,
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let inventory = load_inventory("example_fleet/inventory")?;
    println!("{:#?}", inventory);

    let playbook = load_playbook("example_fleet/playbook.toml")?;
    println!("{:#?}", playbook);

    let ssh_config = Arc::new(russh::client::Config {
        connection_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    });

    let local_agent = LocalAgent::new();
    let mut agent_client = local_agent.connect_client().await;
    //TODO decrypt and load admin keypair in agent

    let public_key = {
        println!("Loading admin key ...");
        let _gpgme = gpgme::init();
        let mut gpg = Context::from_protocol(Protocol::OpenPgp).context("openpgp connect")?;

        let key_path = dirs::home_dir()
            .context("cannot determine homedir")?
            .join(".gander/admin.sec.gpg");

        let key_file = File::open(key_path).context("read admin key")?;

        let mut key_data = Vec::new();
        gpg.decrypt(key_file, &mut key_data)
            .context("decrypt admin key")?;
        let key_data = String::from_utf8(key_data).context("decode admin key")?;
        let key = decode_secret_key(&key_data, None).context("decode admin key")?;
        agent_client
            .add_identity(&key, &[])
            .await
            .context("add_identity")?;

        key.clone_public_key().context("clone_public_key")?
    };

    println!("Connecting to hosts ...");

    let connection_futures = inventory.hosts.iter().map(|host| {
        let ssh_config = Arc::clone(&ssh_config);
        let local_agent = &local_agent;
        let public_key = public_key.clone();
        async move {
            (
                host,
                connect(
                    host,
                    ssh_config,
                    public_key,
                    local_agent.connect_client().await,
                )
                .await,
            )
        }
    });

    let connections = join_all(connection_futures).await;

    for (host, connection_result) in &connections {
        println!("{:?} : {:?}", host.path, connection_result.is_ok());
    }

    Ok(())
}

struct LocalAgent {
    listen_tx: mpsc::Sender<DuplexStream>,
    join_handle: JoinHandle<Result<(), russh_keys::Error>>,
}

impl LocalAgent {
    pub fn new() -> Self {
        let (listen_tx, listen_rx) = mpsc::channel::<DuplexStream>(1);

        let join_handle = tokio::spawn(russh_keys::agent::server::serve(
            ReceiverStream::new(listen_rx).map(Ok),
            (),
        ));

        Self {
            listen_tx,
            join_handle,
        }
    }

    pub async fn connect(&self) -> Option<DuplexStream> {
        let (a, b) = duplex(1024);
        self.listen_tx.send(b).await.ok()?;
        Some(a)
    }

    pub async fn connect_client(&self) -> AgentClient<DuplexStream> {
        AgentClient::connect(self.connect().await.unwrap())
    }
}

async fn connect(
    host: &Arc<HostSpec>,
    ssh_config: Arc<russh::client::Config>,
    public_key: PublicKey,
    agent: AgentClient<DuplexStream>,
) -> anyhow::Result<()> {
    let conn_future = russh::client::connect(
        ssh_config,
        (host.address.as_str(), host.ssh_port),
        Handler {
            host: Arc::clone(&host),
        },
    );
    let connection_result = match timeout(Duration::from_secs(5), conn_future).await {
        Ok(Ok(x)) => Ok(x),
        Ok(Err(err)) => Err(anyhow!(err)),
        Err(err) => Err(anyhow!(err)),
    };
    let mut handle = connection_result.context("connect")?;
    let (agent, result) = handle
        .authenticate_future(&host.ssh_user, public_key, agent)
        .await;
    result.context("authenticate")?;

    let mut channel = handle
        .channel_open_session()
        .await
        .context("channel_open_session")?;
    channel.exec(true, "echo 'Hello World'").await?;

    while let Some(msg) = channel.wait().await {
        println!("{:?}", msg);
        match msg {
            ChannelMsg::Data { data } => {
                println!("{:?}", std::str::from_utf8(&data));
            }
            _ => {}
        }
    }

    Ok(())
}

struct Handler {
    host: Arc<HostSpec>,
}

#[async_trait]
impl russh::client::Handler for Handler {
    type Error = anyhow::Error;

    async fn check_server_key(
        self,
        server_public_key: &PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        //TODO learn known host (prompt user)
        let check_result = russh_keys::check_known_hosts(
            &self.host.address,
            self.host.ssh_port,
            server_public_key,
        );

        let trusted = match check_result {
            Ok(found) => {
                if !found {
                    russh_keys::learn_known_hosts(
                        &self.host.address,
                        self.host.ssh_port,
                        server_public_key,
                    )?;
                }
                true
            }
            Err(russh_keys::Error::KeyChanged { .. }) => false,
            Err(err) => bail!(err),
        };
        Ok((self, trusted))
    }
}
