use std::time::Duration;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use websocket::futures::sync::mpsc::unbounded;

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

static DEFAULT_GROUPS_TOML: &'static [u8] = include_bytes!("./example_groups.toml");

pub fn run() {
  loop {
    if let Some((c, dir)) = find_client().ok() {
      let mut config_dir: PathBuf = [dir, "Config".to_owned()].iter().collect();
      let mut client = LeagueClient::new(c.clone(), config_dir.clone());

      let (client_sender, rx) = unbounded();

      let mut tray = Application::new().unwrap();

      tray.set_icon_from_buffer(ICON, 64, 64).unwrap();

      config_dir.push(".dark-binding");
      config_dir.push("groups.toml");

      let is_editing = Arc::new(AtomicBool::new(false));
      let sender_0 = client_sender.clone();

      tray
        .add_menu_item(&"Edit Champion Groups".to_owned(), move |_| {
          if !is_editing.compare_and_swap(false, true, Ordering::SeqCst) {
            // feelsbadman
            let is_editing = is_editing.clone();
            let groups_toml = config_dir.clone();
            let sender_0 = sender_0.clone();

            let guard = thread::spawn(move || {
              let child = Command::new("notepad.exe")
                .arg(&groups_toml.to_str().unwrap())
                .spawn();

              if let Err(_) = child {
                is_editing.store(false, Ordering::SeqCst);
                
                return;
              };

              let result = child.unwrap().wait();

              if result.is_ok() && result.unwrap().success() {
                &sender_0
                  .unbounded_send(LeagueClientFn::ReloadGroups)
                  .expect("Couldn't reload groups");
              }
              
              is_editing.store(false, Ordering::SeqCst);
            });
          }
        })
        .ok();

      let sender_1 = client_sender.clone();

      tray
        .add_menu_item(&"Backup Config".to_string(), move |_| {
          sender_1
            .unbounded_send(LeagueClientFn::BackupConfig)
            .expect("Couldn't backup config");
        })
        .ok();

      let sender_2 = client_sender.clone();

      tray
        .add_menu_item(&"Restore Config".to_string(), move |_| {
          sender_2
            .unbounded_send(LeagueClientFn::RestoreConfig)
            .expect("Couldn't restore config");
        })
        .ok();

      tray.add_menu_separator().ok();

      let sender_3 = client_sender.clone();

      tray
        .add_menu_item(&"Quit".to_string(), move |sender| {
          sender_3.unbounded_send(LeagueClientFn::Shutdown).unwrap();
          sender.send(SystrayEvent::Quit).ok();
        })
        .ok();

      let tray_sender = tray.wait_for_message();

      let result = client.init(rx);

      tray_sender.send(SystrayEvent::Quit).ok();

      break;
    }

    thread::sleep(Duration::from_secs(60));
  }
}
