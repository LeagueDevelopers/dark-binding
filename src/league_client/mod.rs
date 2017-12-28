use std::time::Duration;
use std::thread;
use websocket::WebSocketError;
use websocket::futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};

use errors::*;

mod structs;
mod util;
mod websocket;
mod client;

use systray::{Application, SystrayEvent};

use self::client::*;
use self::util::find_client;
pub use self::structs::*;

static ICON: &'static [u8] = include_bytes!("../../resources/dark-binding.ico");

pub fn run() {
  loop {
    if let Some((c, dir)) = find_client().ok() {
      let mut client = LeagueClient::new(c.clone(), dir);

      let (client_sender, rx) = unbounded();

      let mut tray = Application::new().unwrap();

      tray.set_icon_from_buffer(ICON, 64, 64).unwrap();

      let sender_1 = client_sender.clone();

      tray
        .add_menu_item(
          &"Backup Config".to_string(), move |_| {
            sender_1
              .unbounded_send(LeagueClientFn::BackupConfig)
              .expect("Couldn't backup config");
          }
        )
        .ok();

      let sender_2 = client_sender.clone();

      tray
        .add_menu_item(
          &"Restore Config".to_string(), move |_| {
            sender_2
              .unbounded_send(LeagueClientFn::RestoreConfig)
              .expect("Couldn't restore config");
          }
        )
        .ok();

      tray.add_menu_separator().ok();

      let sender_3 = client_sender.clone();

      tray
        .add_menu_item(
          &"Quit".to_string(), move |sender| {
            sender_3
              .unbounded_send(LeagueClientFn::Shutdown)
              .unwrap();
            sender.send(SystrayEvent::Quit).ok();
          }
        )
        .ok();

      let tray_sender = tray.wait_for_message();

      let result = client.init(rx);

      tray_sender.send(SystrayEvent::Quit).ok();

      break;
    }

    thread::sleep(Duration::from_secs(60));
  }
}
