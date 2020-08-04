use super::{ButtplugServer, ButtplugServerStartupError};
use crate::{
  connector::ButtplugConnector,
  core::{
    errors::{ButtplugError, ButtplugServerError},
    messages::{self, ButtplugClientMessage, ButtplugServerMessage},
  },
  server::{DeviceCommunicationManager, DeviceCommunicationManagerCreator},
  test::TestDeviceCommunicationManagerHelper,
  util::async_manager,
};
use async_channel::{bounded, Receiver, Sender};
use async_mutex::Mutex;
use futures::{future::Future, select, FutureExt, StreamExt};
use std::sync::Arc;
use thiserror::Error;

pub enum ButtplugServerEvent {
  Connected(String),
  DeviceAdded(String),
  DeviceRemoved(String),
  Disconnected,
}

#[derive(Error, Debug)]
pub enum ButtplugServerConnectorError {
  #[error("Can't connect")]
  ConnectorError,
}

pub enum ButtplugServerCommand {
  Disconnect,
}

pub struct ButtplugRemoteServer {
  server: Arc<ButtplugServer>,
  server_receiver: Receiver<ButtplugServerMessage>,
  task_channel: Arc<Mutex<Option<Sender<ButtplugServerCommand>>>>,
}

async fn run_server<ConnectorType>(
  server: Arc<ButtplugServer>,
  mut server_receiver: Receiver<ButtplugServerMessage>,
  connector: ConnectorType,
  mut connector_receiver: Receiver<Result<ButtplugClientMessage, ButtplugServerError>>,
  mut controller_receiver: Receiver<ButtplugServerCommand>,
) where
  ConnectorType: ButtplugConnector<ButtplugServerMessage, ButtplugClientMessage> + 'static,
{
  info!("Starting remote server loop");
  let shared_connector = Arc::new(connector);
  loop {
    select! {
      connector_msg = connector_receiver.next().fuse() => match connector_msg {
        None => {
          info!("Connector disconnected, exiting loop.");
          break;
        }
        Some(msg) => {
          info!("Got message from connector: {:?}", msg);
          let server_clone = server.clone();
          let connector_clone = shared_connector.clone();
          async_manager::spawn(async move {
            match server_clone.parse_message(msg.unwrap()).await {
              Ok(ret_msg) => {
                if connector_clone.send(ret_msg).await.is_err() {
                  error!("Cannot send reply to server, dropping and assuming remote server thread has exited.")
                }
              },
              Err(err_msg) => {
                if connector_clone.send(messages::Error::from(err_msg).into()).await.is_err() {
                  error!("Cannot send reply to server, dropping and assuming remote server thread has exited.")
                }
              }
            }
          }).unwrap();
        }
      },
      controller_msg = controller_receiver.next().fuse() => match controller_msg {
        None => {
          info!("Server disconnected via controller request, exiting loop.");
          break;
        }
        Some(msg) => {
          info!("Server disconnected via controller disappearance, exiting loop.");
          break;
        }
      },
      server_msg = server_receiver.next().fuse() => match server_msg {
        None => {
          info!("Server disconnected via server disappearance, exiting loop.");
          break;
        }
        Some(msg) => {
          let connector_clone = shared_connector.clone();
          if connector_clone.send(msg).await.is_err() {
            error!("Server disappeared, exiting remote server thread.");
            break;
          }
        }
      },
    };
  }
  if let Err(err) = server.disconnect().await {
    error!("Error disconnecting server: {:?}", err);
  }
  info!("Exiting remote server loop");
}

impl ButtplugRemoteServer {
  pub fn new(name: &str, max_ping_time: u64) -> Self {
    let (server, server_receiver) = ButtplugServer::new(name, max_ping_time);
    Self {
      server: Arc::new(server),
      server_receiver,
      task_channel: Arc::new(Mutex::new(None)),
    }
  }

  pub fn start<ConnectorType>(
    &self,
    mut connector: ConnectorType,
  ) -> impl Future<Output = Result<(), ButtplugServerConnectorError>>
  where
    ConnectorType: ButtplugConnector<ButtplugServerMessage, ButtplugClientMessage> + 'static,
  {
    let task_channel = self.task_channel.clone();
    let server_clone = self.server.clone();
    let server_receiver_clone = self.server_receiver.clone();
    async move {
      let connector_receiver = connector
        .connect()
        .await
        .map_err(|_| ButtplugServerConnectorError::ConnectorError)?;
      let (controller_sender, controller_receiver) = bounded(256);
      let mut locked_channel = task_channel.lock().await;
      *locked_channel = Some(controller_sender);
      run_server(
        server_clone,
        server_receiver_clone,
        connector,
        connector_receiver,
        controller_receiver,
      )
      .await;
      Ok(())
    }
  }

  pub async fn disconnect(&self) -> Result<(), ButtplugError> {
    Ok(())
  }

  pub fn add_comm_manager<T>(&self) -> Result<(), ButtplugServerStartupError>
  where
    T: 'static + DeviceCommunicationManager + DeviceCommunicationManagerCreator,
  {
    self.server.add_comm_manager::<T>()
  }

  pub fn add_test_comm_manager(&self) -> Result<TestDeviceCommunicationManagerHelper, ButtplugServerStartupError> {
    self.server.add_test_comm_manager()
  }
}