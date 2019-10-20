use crate::core::{
    errors::{ButtplugError, ButtplugMessageError},
    messages::{
        ButtplugMessage, ButtplugMessageUnion, DeviceAdded, DeviceMessageInfo,
        MessageAttributes, VibrateCmd, VibrateSubcommand,
    },
};
use super::internal::{ButtplugInternalClientMessage, ButtplugClientMessageFuture};
use futures::SinkExt;
use futures_channel::mpsc;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ButtplugClientDevice {
    pub name: String,
    index: u32,
    pub allowed_messages: HashMap<String, MessageAttributes>,
    client_sender: mpsc::UnboundedSender<ButtplugInternalClientMessage>,
}

impl ButtplugClientDevice {
    pub fn new(
        name: &str,
        index: u32,
        allowed_messages: HashMap<String, MessageAttributes>,
        client_sender: mpsc::UnboundedSender<ButtplugInternalClientMessage>,
    ) -> ButtplugClientDevice {
        ButtplugClientDevice {
            name: name.to_owned(),
            index,
            allowed_messages,
            client_sender,
        }
    }

    async fn send_message(&mut self, msg: ButtplugMessageUnion) -> ButtplugMessageUnion {
        let id = msg.get_id();
        let fut = ButtplugClientMessageFuture::default();
        self.client_sender.send(
            ButtplugInternalClientMessage::Message((msg.clone(), fut.get_state_ref().clone()))).await;
        fut.await
    }

    async fn send_message_expect_ok(&mut self, msg: ButtplugMessageUnion) -> Option<ButtplugError> {
        let msg = self.send_message(msg).await;
        match msg {
            ButtplugMessageUnion::Ok(_) => None,
            ButtplugMessageUnion::Error(_err) => Some(ButtplugError::from(_err)),
            _ => Some(ButtplugError::ButtplugMessageError(ButtplugMessageError {
                message: "Got unexpected message type.".to_owned(),
            })),
        }
    }

    pub async fn send_vibrate_cmd(&mut self, speed: f64) -> Option<ButtplugError> {
        self.send_message_expect_ok(ButtplugMessageUnion::VibrateCmd(VibrateCmd::new(
            self.index,
            vec![VibrateSubcommand::new(0, speed)],
        )))
        .await
    }

    // pub async fn send_linear_cmd(&self) -> Option<ButtplugError> {
    //     None
    // }

    // pub async fn send_rotation_cmd(&self) -> Option<ButtplugError> {
    //     None
    // }
}

impl
    From<(
        &DeviceAdded,
        mpsc::UnboundedSender<ButtplugInternalClientMessage>,
    )> for ButtplugClientDevice
{
    fn from(
        msg_sender_tuple: (
            &DeviceAdded,
            mpsc::UnboundedSender<ButtplugInternalClientMessage>,
        ),
    ) -> Self {
        let msg = msg_sender_tuple.0.clone();
        ButtplugClientDevice::new(
            &*msg.device_name,
            msg.device_index,
            msg.device_messages,
            msg_sender_tuple.1,
        )
    }
}

impl
    From<(
        &DeviceMessageInfo,
        mpsc::UnboundedSender<ButtplugInternalClientMessage>,
    )> for ButtplugClientDevice
{
    fn from(
        msg_sender_tuple: (
            &DeviceMessageInfo,
            mpsc::UnboundedSender<ButtplugInternalClientMessage>,
        ),
    ) -> Self {
        let msg = msg_sender_tuple.0.clone();
        ButtplugClientDevice::new(
            &*msg.device_name,
            msg.device_index,
            msg.device_messages,
            msg_sender_tuple.1,
        )
    }
}
