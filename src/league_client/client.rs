use std::fs;
use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use reqwest::Response;
use serde::ser::Serialize;
use native_tls::{TlsConnector, Certificate};
use websocket::{WebSocketError, Message, OwnedMessage};
use websocket::ClientBuilder;
use websocket::futures::{Future, Stream, Sink};
use websocket::header::{Headers, Authorization, Basic};
use websocket::futures::sync::mpsc::UnboundedReceiver;
use tokio_core::reactor::Core;

use CERTIFICATE;
use errors::*;

use league_client::structs::*;
use league_client::util::*;
use league_client::websocket::LeagueSocketHandler;

pub enum LeagueClientFn {
  BackupConfig,
  RestoreConfig,
  Shutdown,
  Message(OwnedMessage)
}

#[derive(Clone)]
pub struct Credentials {
  pub pid: u32,
  pub port: String,
  pub token: String
}

pub struct LeagueClient {
  credentials: Credentials,
  config_folder: PathBuf,
  region: Option<String>,
  local_summoner: Option<LocalSummoner>
}

impl LeagueClient {
  pub fn new(credentials: Credentials, install_directory: String) -> LeagueClient {
    LeagueClient {
      credentials: credentials,
      config_folder: [install_directory, "Config".to_owned()].iter().collect(),
      region: None,
      local_summoner: None
    }
  }

  fn persisted_settings(&self) -> PathBuf {
    self.config_folder.join("PersistedSettings.json")
  }

  fn persisted_settings_backup(&self) -> PathBuf {
    let mut p = self.persisted_settings();
    p.set_extension("bak");

    p
  }

  pub fn local_summoner(&self) -> Option<LocalSummoner> {
    match self.local_summoner {
      Some(ref s) => Some(s.clone()),
      _ => None,
    }
  }

  fn create_config(&self, config_path: &Path) -> Result<()> {
    self.restore_config()?;
    let persisted_settings_loc = self.persisted_settings();

    ensure_dir(config_path)?;
    fs::copy(persisted_settings_loc, config_path);

    Ok(())
  }

  pub fn backup_config(&self) -> Result<()> {
    let persisted_settings = self.persisted_settings();
    let backup = self.persisted_settings_backup();

    if persisted_settings.exists() && !is_symlink(&persisted_settings)? {
      fs::copy(persisted_settings, backup)?;
    }

    Ok(())
  }

  pub fn restore_config(&self) -> Result<()> {
    let current_settings_loc = self.persisted_settings();
    let backup_loc = self.persisted_settings_backup();

    let real_loc = match current_settings_loc.read_link() {
      Ok(location) => location,
      _ => {
        // it's not one of our hard links, nothing left to do
        return Ok(());
      }
    };

    if !backup_loc.exists() {
      fs::remove_file(&current_settings_loc)?;
      fs::copy(real_loc, current_settings_loc)?;

      return Ok(());
    }

    let current_settings = read_settings_json(&real_loc)?;
    let next_game_cfg = current_settings
      .files
      .into_iter()
      .find(|f| f.name == "Game.cfg");

    match next_game_cfg {
      Some(cfg) => {
        let mut final_settings = read_settings_json(&backup_loc)?;

        for file in final_settings.files.iter_mut() {
          if file.name == "Game.cfg" {
            *file = cfg;

            break;
          }
        }

        write_settings_json(&backup_loc, &final_settings)?;
      }
      _ => {}
    };

    fs::remove_file(&current_settings_loc);
    fs::rename(&backup_loc, &current_settings_loc)?;

    Ok(())
  }

  pub fn load_champion_config(&self, champion_id: u32) -> Result<()> {
    self.restore_config()?;

    let mut cfg_file_loc = self.config_folder.join(".dark-binding");

    cfg_file_loc.set_file_name(champion_id.to_string());
    cfg_file_loc.set_extension("json");

    if !cfg_file_loc.exists() {
      self.create_config(&cfg_file_loc);
    }

    let persisted_settings_loc = self.persisted_settings();

    self.backup_config()?;

    if persisted_settings_loc.exists() {
      fs::remove_file(&persisted_settings_loc);
    }

    fs::hard_link(cfg_file_loc, persisted_settings_loc);

    Ok(())
  }

  fn update_local_summoner(&mut self) -> Result<()> {
    let local_summoner: LocalSummoner =
      self
        .get("/lol-summoner/v1/current-summoner", None::<&[(String, String)]>)?
        .json()
        .chain_err(|| "unable to get local summoner, check if you're logged in")?;

    self.local_summoner = Some(local_summoner);

    Ok(())
  }

  fn get<I, K, V>(&self, endpoint: &str, query: Option<I>) -> Result<Response>
    where I: IntoIterator,
          I::Item: Borrow<(K, V)>,
          K: AsRef<str>,
          V: AsRef<str> {
    base_get(endpoint, &self.credentials, query).and_then(|res| res.error_for_status().map_err(|e| e.into()))
  }

  fn post<T: Serialize>(&self, endpoint: &str, json: &T) -> Result<Response> {
    base_post(endpoint, &self.credentials, json).and_then(|res| res.error_for_status().map_err(|e| e.into()))
  }

  fn connect(&mut self, rx: UnboundedReceiver<LeagueClientFn>) -> Result<()> {
    let mut core = Core::new().unwrap();

    let tls_connector = TlsConnector::builder()
      .unwrap()
      .what_the_fuck(Certificate::from_der(CERTIFICATE).unwrap())
      .ok();

    let (url, headers) = {
      let c = &self.credentials;
      let url = format!("wss://riot:{}@127.0.0.1:{}", c.token, c.port);

      let mut headers = Headers::new();

      headers.set(
        Authorization(
          Basic {
            username: "riot".to_owned(),
            password: Some(c.token.to_owned())
          }
        )
      );

      (url, headers)
    };

    debug!("Connecting to {}", url);

    let f = ClientBuilder::new(&url)
      .unwrap()
      .add_protocol("wamp")
      .custom_headers(&headers)
      .async_connect_secure(tls_connector, &core.handle())
      .and_then(|(duplex, _)| duplex.send(Message::text("[5,\"OnJsonApiEvent\"]").into()))
      .and_then(
        |duplex| {
          let (_s, stream) = duplex.split();

          stream
            .map(|m| LeagueClientFn::Message(m))
            .select(rx.map_err(|_| WebSocketError::NoDataAvailable))
            .for_each(
              |message| {
                match message {
                  LeagueClientFn::BackupConfig => {
                    self.backup_config().ok();
                  }
                  LeagueClientFn::RestoreConfig => {
                    self.restore_config().ok();
                  }
                  LeagueClientFn::Shutdown => {
                    self.shutdown().ok();
                    return Err(WebSocketError::NoDataAvailable);
                  }
                  LeagueClientFn::Message(m) => {
                    self.on_message(m);
                  }
                  _ => {}
                };

                Ok(())
              }
            )
        }
      );

    core.run(f)?;

    Ok(())
  }
  pub fn init(&mut self, rx: UnboundedReceiver<LeagueClientFn>) -> Result<()> {
    let rso_auth: RSO =
      self
        .get("/rso-auth/v1/authorization", None::<&[(String, String)]>)?
        .json()
        .chain_err(|| "unable to get summoner region, check if you're logged in")?;

    debug!("RSO auth successful: {:?}", rso_auth);

    self.region = Some(rso_auth.current_platform_id);

    self.connect(rx)
  }

  pub fn shutdown(&self) -> Result<()> {
    debug!("Shutting down client forcefully");

    Ok(())
  }
}
