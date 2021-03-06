use std::{fs, os};
use std::io::Write;
use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use reqwest::Response;
use native_tls::{Certificate, TlsConnector};
use websocket::{Message, OwnedMessage, WebSocketError};
use websocket::ClientBuilder;
use websocket::futures::{Future, Sink, Stream};
use websocket::header::{Authorization, Basic, Headers};
use websocket::futures::sync::mpsc::UnboundedReceiver;
use tokio_core::reactor::Core;

use CERTIFICATE;
use errors::*;

use league_client::DEFAULT_GROUPS_TOML;
use league_client::structs::*;
use league_client::util::*;
use league_client::websocket::LeagueSocketHandler;

pub enum LeagueClientFn {
  BackupConfig,
  RestoreConfig,
  ReloadGroups,
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
  local_summoner: Option<LocalSummoner>,
  champion_names: HashMap<String, i32>,
  champion_groups: HashMap<i32, String>
}

impl LeagueClient {
  pub fn new(credentials: Credentials, config_directory: PathBuf) -> LeagueClient {
    LeagueClient {
      credentials: credentials,
      config_folder: config_directory,
      region: None,
      local_summoner: None,
      champion_names: HashMap::new(),
      champion_groups: HashMap::new()
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
      _ => None
    }
  }

  /// Reload champion group associations from file
  fn update_champion_groups(&mut self) -> Result<()> {
    let groups_toml = self.config_folder.join(".dark-binding").join("groups.toml");

    if !groups_toml.exists() {
      create_default_groups_toml(groups_toml.parent().unwrap());
    }

    let groups = read_toml(&groups_toml)?.groups;

    self.champion_groups.clear();

    groups.iter().for_each(|(group_name, champions)| {
      champions
        .iter()
        .map(|name| normalize_champion_name(name))
        .for_each(|champion| {
          if let Some(champion_id) = self.champion_names.get(&champion) {
            debug!(
              "Adding {} (ID {}) to group {}",
              champion, champion_id, group_name
            );
            self
              .champion_groups
              .insert(*champion_id, group_name.to_owned());
          }
        });
    });

    Ok(())
  }

  /// Backs up the existing PersistedSettings.json if it is the default one
  /// ex: doesn't belong to any group
  pub fn backup_config(&self) -> Result<()> {
    debug!("backing up config");
    let persisted_settings = self.persisted_settings();
    let backup = self.persisted_settings_backup();

    if persisted_settings.exists() && !is_symlink(&persisted_settings)? {
      fs::copy(persisted_settings, backup)?;
    }

    Ok(())
  }

  /// Restores the original PersistedSettings.json and sync any non-keybind
  /// settings changes
  pub fn restore_config(&self) -> Result<()> {
    let current_settings_loc = self.persisted_settings();
    let backup_loc = self.persisted_settings_backup();

    if backup_loc.exists() && !current_settings_loc.exists() {
      debug!("backup found but no current config");

      fs::copy(&backup_loc, &current_settings_loc)?;
      fs::remove_file(&backup_loc);
    }

    let real_loc = match current_settings_loc.read_link() {
      Ok(location) => location,
      _ => {
        debug!("not a symlink, aborting");
        // it's not one of our links, nothing left to do
        return Ok(());
      }
    };

    debug!("restoring currently symlinked config {:?}", real_loc);

    // Something went wrong and the backup got deleted, make whatever
    // config is currently loaded the permanent one
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
    fs::rename(backup_loc, current_settings_loc)?;

    Ok(())
  }

  /// Load config for a champion if it belongs to a group
  ///
  /// Creates a new config file if it doesn't exist already
  pub fn load_champion_config(&self, champion_id: i32) -> Result<()> {
    self.restore_config()?;

    let group_name = match self.champion_groups.get(&champion_id) {
      Some(n) => n,
      _ => return Ok(())
    };

    debug!("loading group name: {:?}", group_name);

    let mut cfg_file_loc = self.config_folder.join(".dark-binding").join(group_name);
    cfg_file_loc.set_extension("json");

    let persisted_settings_loc = self.persisted_settings();

    debug!("loading champion config {:?}", cfg_file_loc);

    if !cfg_file_loc.exists() {
      debug!("copying from {:?} to {:?}", persisted_settings_loc, cfg_file_loc);

      ensure_dir(&cfg_file_loc.parent().unwrap())?;
      fs::copy(&persisted_settings_loc, &cfg_file_loc)?;
    }

    self.backup_config()?;

    if persisted_settings_loc.exists() {
      fs::remove_file(&persisted_settings_loc);
    }

    debug!("symlinking from {:?} to {:?}", cfg_file_loc, persisted_settings_loc);
    // if cfg!(windows) {
    os::windows::fs::symlink_file(cfg_file_loc, persisted_settings_loc).unwrap();
    // } else {
      // os::unix::fs::symlink(cfg_file_loc, persisted_settings_loc)?;
    // }

    Ok(())
  }

  fn update_local_structs(&mut self) -> Result<()> {
    let local_summoner: LocalSummoner = self
      .get(
        "/lol-summoner/v1/current-summoner",
        None::<&[(String, String)]>
      )?
      .json()
      .chain_err(|| "unable to get local summoner, check if you're logged in")?;


    let champions: Vec<ChampionMinimal> = self
      .get(
        &format!(
          "/lol-champions/v1/inventories/{}/champions-minimal",
          &local_summoner.summoner_id
        ),
        None::<&[(String, String)]>
      )?
      .json()
      .chain_err(|| "unable to get champion list from client")?;

    champions.iter().filter(|e| e.id > 0).for_each(|e| {
      self
        .champion_names
        .insert(normalize_champion_name(&e.alias), e.id);
    });

    self.local_summoner = Some(local_summoner);
    self.update_champion_groups()?;

    Ok(())
  }

  fn get<I, K, V>(&self, endpoint: &str, query: Option<I>) -> Result<Response>
  where
    I: IntoIterator,
    I::Item: Borrow<(K, V)>,
    K: AsRef<str>,
    V: AsRef<str>
  {
    base_get(endpoint, &self.credentials, query)
      .and_then(|res| res.error_for_status().map_err(|e| e.into()))
  }

  // fn post<T: Serialize>(&self, endpoint: &str, json: &T) -> Result<Response> {
  //   base_post(endpoint, &self.credentials, json)
  //     .and_then(|res| res.error_for_status().map_err(|e| e.into()))
  // }

  fn connect(&mut self, rx: UnboundedReceiver<LeagueClientFn>) -> Result<()> {
    self.update_local_structs();

    let mut core = Core::new().unwrap();

    let tls_connector = TlsConnector::builder()
      .unwrap()
      .what_the_fuck(Certificate::from_der(CERTIFICATE).unwrap())
      .ok();

    let (url, headers) = {
      let c = &self.credentials;
      let url = format!("wss://riot:{}@127.0.0.1:{}", c.token, c.port);

      let mut headers = Headers::new();

      headers.set(Authorization(Basic {
        username: "riot".to_owned(),
        password: Some(c.token.to_owned())
      }));

      (url, headers)
    };

    debug!("Connecting to {}", url);

    let f = ClientBuilder::new(&url)
      .unwrap()
      .add_protocol("wamp")
      .custom_headers(&headers)
      .async_connect_secure(tls_connector, &core.handle())
      .and_then(|(duplex, _)| duplex.send(Message::text("[5,\"OnJsonApiEvent\"]").into()))
      .and_then(|duplex| {
        let (_s, stream) = duplex.split();

        stream
          .map(|m| LeagueClientFn::Message(m))
          .select(rx.map_err(|_| WebSocketError::NoDataAvailable))
          .for_each(|message| {
            match message {
              LeagueClientFn::BackupConfig => {
                self.backup_config();
              }
              LeagueClientFn::RestoreConfig => {
                self.restore_config();
              }
              LeagueClientFn::ReloadGroups => {
                self.update_champion_groups();
              }
              LeagueClientFn::Shutdown => {
                self.shutdown();
                return Err(WebSocketError::NoDataAvailable);
              }
              LeagueClientFn::Message(m) => {
                self.on_message(m);
              }
            };

            Ok(())
          })
      });

    core.run(f)?;

    Ok(())
  }

  pub fn init(&mut self, rx: UnboundedReceiver<LeagueClientFn>) -> Result<()> {
    let rso_auth: RSO = self
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

/// Copies the included default groups.toml to the .dark-binding config directory
fn create_default_groups_toml(path: &Path) -> Result<()> {
  ensure_dir(path)?;

  Ok(fs::OpenOptions::new().write(true).create(true).open(path.join("groups.toml"))?.write_all(DEFAULT_GROUPS_TOML)?)
}
