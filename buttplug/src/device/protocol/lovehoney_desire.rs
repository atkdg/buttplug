use super::{ButtplugDeviceResultFuture, ButtplugProtocol, ButtplugProtocolCommandHandler};
use crate::{
  core::messages::{self, ButtplugDeviceCommandMessageUnion, DeviceMessageAttributesMap},
  device::{
    protocol::{generic_command_manager::GenericCommandManager, ButtplugProtocolProperties},
    DeviceImpl, DeviceWriteCmd, Endpoint,
  },
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(ButtplugProtocolProperties)]
pub struct LovehoneyDesire {
  name: String,
  message_attributes: DeviceMessageAttributesMap,
  manager: Arc<Mutex<GenericCommandManager>>,
  stop_commands: Vec<ButtplugDeviceCommandMessageUnion>,
}

impl ButtplugProtocol for LovehoneyDesire {
  fn new_protocol(
    name: &str,
    message_attributes: DeviceMessageAttributesMap,
  ) -> Box<dyn ButtplugProtocol> {
    let manager = GenericCommandManager::new(&message_attributes);

    Box::new(Self {
      name: name.to_owned(),
      message_attributes,
      stop_commands: manager.get_stop_commands(),
      manager: Arc::new(Mutex::new(manager)),
    })
  }
}

impl ButtplugProtocolCommandHandler for LovehoneyDesire {
  fn handle_vibrate_cmd(
    &self,
    device: Arc<DeviceImpl>,
    message: messages::VibrateCmd,
  ) -> ButtplugDeviceResultFuture {
    // Store off result before the match, so we drop the lock ASAP.
    let manager = self.manager.clone();
    Box::pin(async move {
      let result = manager.lock().await.update_vibration(&message, false)?;
      if let Some(cmds) = result {
        // The Lovehoney Desire has 2 types of commands
        //
        // - Set both motors with one command
        // - Set each motor separately
        //
        // We'll need to check what we got back and write our
        // commands accordingly.
        //
        // Neat way of checking if everything is the same via
        // https://sts10.github.io/2019/06/06/is-all-equal-function.html.
        //
        // Just make sure we're not matching on None, 'cause if
        // that's the case we ain't got shit to do.
        let mut fut_vec = vec![];
        if cmds[0].is_some() && cmds.windows(2).all(|w| w[0] == w[1]) {
          let fut = device.write_value(DeviceWriteCmd::new(
            Endpoint::Tx,
            vec![0xF3, 0, cmds[0].unwrap() as u8],
            false,
          ));
          fut.await?;
        } else {
          // We have differening values. Set each motor separately.
          let mut i = 1;

          for cmd in cmds {
            if let Some(speed) = cmd {
              fut_vec.push(device.write_value(DeviceWriteCmd::new(
                Endpoint::Tx,
                vec![0xF3, i, speed as u8],
                false,
              )));
            }
            i += 1;
          }
          for fut in fut_vec {
            fut.await?;
          }
        }
      }
      Ok(messages::Ok::default().into())
    })
  }
}

#[cfg(all(test, feature = "server"))]
mod test {
  use crate::{
    core::messages::{StopDeviceCmd, VibrateCmd, VibrateSubcommand},
    device::{DeviceImplCommand, DeviceWriteCmd, Endpoint},
    server::comm_managers::test::{check_test_recv_empty, check_test_recv_value, new_bluetoothle_test_device},
    util::async_manager,
  };

  #[test]
  pub fn test_lovehoney_desire_protocol() {
    async_manager::block_on(async move {
      let (device, test_device) = new_bluetoothle_test_device("PROSTATE VIBE").await.unwrap();
      let command_receiver = test_device.get_endpoint_receiver(&Endpoint::Tx).unwrap();

      // If we send one speed to one motor, we should only see one output.
      device
        .parse_message(VibrateCmd::new(0, vec![VibrateSubcommand::new(0, 0.5)]).into())
        .await
        .unwrap();
      check_test_recv_value(
        &command_receiver,
        DeviceImplCommand::Write(DeviceWriteCmd::new(
          Endpoint::Tx,
          vec![0xF3, 0x1, 0x40],
          false,
        )),
      );
      assert!(check_test_recv_empty(&command_receiver));

      // If we send the same speed to each motor, we should only get one command.
      device
        .parse_message(
          VibrateCmd::new(
            0,
            vec![
              VibrateSubcommand::new(0, 0.1),
              VibrateSubcommand::new(1, 0.1),
            ],
          )
          .into(),
        )
        .await
        .unwrap();
      check_test_recv_value(
        &command_receiver,
        DeviceImplCommand::Write(DeviceWriteCmd::new(
          Endpoint::Tx,
          vec![0xF3, 0x0, 0x0d],
          false,
        )),
      );
      assert!(check_test_recv_empty(&command_receiver));

      // If we send different commands to both motors, we should get 2 different commands, each with an index.
      device
        .parse_message(
          VibrateCmd::new(
            0,
            vec![
              VibrateSubcommand::new(0, 0.0),
              VibrateSubcommand::new(1, 0.5),
            ],
          )
          .into(),
        )
        .await
        .unwrap();
      check_test_recv_value(
        &command_receiver,
        DeviceImplCommand::Write(DeviceWriteCmd::new(
          Endpoint::Tx,
          vec![0xF3, 0x01, 0x00],
          false,
        )),
      );
      check_test_recv_value(
        &command_receiver,
        DeviceImplCommand::Write(DeviceWriteCmd::new(
          Endpoint::Tx,
          vec![0xF3, 0x02, 0x40],
          false,
        )),
      );
      assert!(check_test_recv_empty(&command_receiver));

      device
        .parse_message(StopDeviceCmd::new(0).into())
        .await
        .unwrap();
      check_test_recv_value(
        &command_receiver,
        DeviceImplCommand::Write(DeviceWriteCmd::new(
          Endpoint::Tx,
          vec![0xF3, 0x02, 0x0],
          false,
        )),
      );
      assert!(check_test_recv_empty(&command_receiver));
    });
  }
}
