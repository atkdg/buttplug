pub mod lovense_dongle_device_impl;
mod lovense_dongle_messages;
mod lovense_dongle_state_machine;
pub mod lovense_hid_dongle_comm_manager;
pub mod lovense_serial_dongle_comm_manager;

pub use lovense_dongle_device_impl::{LovenseDongleDeviceImpl, LovenseDongleDeviceImplCreator};
pub use lovense_hid_dongle_comm_manager::{
  LovenseHIDDongleCommunicationManager, LovenseHIDDongleCommunicationManagerBuilder,
};
pub use lovense_serial_dongle_comm_manager::{
  LovenseSerialDongleCommunicationManager, LovenseSerialDongleCommunicationManagerBuilder,
};
