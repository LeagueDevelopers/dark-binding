use std::{fs, io};
use std::io::prelude::*;
use std::fs::{File, OpenOptions};
use std::path::Path;
use reqwest::{Response, Url};
use regex::Regex;
use std::process::{Command, Stdio};
use std::borrow::Borrow;
use serde::ser::Serialize;
use serde_json;

use errors::*;
use HTTP_CLIENT;

use league_client::*;
use league_client::structs::PersistedSettings;

lazy_static! {
  static ref PID_REGEX: Regex = Regex::new("--app-pid=(\\d+)\"").unwrap();
  static ref AUTH_REGEX: Regex = Regex::new("--remoting-auth-token=(\\S+)\"").unwrap();
  static ref PORT_REGEX: Regex = Regex::new("--app-port=(\\d+)").unwrap();
  static ref INSTALL_DIR_REGEX: Regex = Regex::new("--install-directory=([^\"]+)\"").unwrap();
}

pub fn parse_credentials(str_to_match: String) -> Result<(Credentials, String)> {
  let pid: u32 = PID_REGEX
    .captures(&str_to_match)
    .chain_err(|| "couldn't find app pid")?
    .get(1)
    .map(|m| m.as_str())
    .chain_err(|| "couldn't parse app-pid")?
    .parse()?;

  let app_port = PORT_REGEX
    .captures(&str_to_match)
    .chain_err(|| "couldn't find app port")?
    .get(1)
    .map(|m| m.as_str())
    .chain_err(|| "couldn't parse app-port")?;

  let password = AUTH_REGEX
    .captures(&str_to_match)
    .chain_err(|| "couldn't find auth token")?
    .get(1)
    .map(|m| m.as_str())
    .chain_err(|| "couldn't parse auth token")?;

  let install_directory = INSTALL_DIR_REGEX
    .captures(&str_to_match)
    .chain_err(|| "couldn't find install directory")?
    .get(1)
    .map(|m| m.as_str())
    .chain_err(|| "couldn't parse isntall directory")?;

  debug!(
    "Obtained credentials: {{ pid: {:?}, port: {:?}, token: {:?}, install_directory: {:?} }}",
    pid,
    app_port,
    password,
    install_directory
  );

  Ok(
    (Credentials {
       pid: pid,
       port: app_port.to_owned(),
       token: password.to_owned()
     },
     install_directory.to_owned())
  )
}

pub fn find_client() -> Result<(Credentials, String)> {
  let child = Command::new("WMIC")
    .args(
      &["PROCESS",
        "WHERE",
        "name='LeagueClientUx.exe'",
        "GET",
        "commandline"]
    )
    .stdout(Stdio::piped())
    .spawn()
    .chain_err(|| "could not initialize WMIC")?;

  let child_out = child
    .wait_with_output()
    .chain_err(|| "failed to get WMIC response")?;

  let output = String::from_utf8_lossy(&child_out.stdout).into_owned();
  // debug!("get_client: {:?}", output);

  Ok(parse_credentials(output)?)
}

pub fn build_uri<I, K, V>(endpoint: &str,
                          credentials: &Credentials,
                          query: Option<I>)
                          -> Result<Url>
  where I: IntoIterator,
        I::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str> {
  let uri = match endpoint.starts_with("/") {
    true => format!("https://127.0.0.1:{}{}", credentials.port, endpoint),
    false => format!("https://127.0.0.1:{}/{}", credentials.port, endpoint),
  };

  if let Some(opts) = query {
    return Ok(Url::parse_with_params(&uri, opts)?);
  }

  Ok(Url::parse(&uri)?)
}

pub fn base_get<I, K, V>(endpoint: &str,
                         credentials: &Credentials,
                         query: Option<I>)
                         -> Result<Response>
  where I: IntoIterator,
        I::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str> {
  let uri = build_uri(endpoint, credentials, query)?;

  Ok(
    HTTP_CLIENT
      .get(uri)
      .basic_auth("riot", Some(credentials.token.to_owned()))
      .send()?
  )
}

pub fn base_post<T: Serialize>(endpoint: &str,
                               credentials: &Credentials,
                               json: &T)
                               -> Result<Response> {
  let uri: Url = build_uri(endpoint, credentials, None::<&[(String, String)]>)?;

  Ok(
    HTTP_CLIENT
      .post(uri)
      .json(json)
      .basic_auth("riot", Some(credentials.token.to_owned()))
      .send()?
  )
}

pub fn is_symlink(path: &Path) -> Result<bool> {
  Ok(fs::metadata(path)?.file_type().is_symlink())
}

pub fn ensure_dir(path: &Path) -> Result<()> {
  if let Err(e) = fs::metadata(path) {
    match e.kind() {
      io::ErrorKind::NotFound => {
        fs::create_dir_all(path)?;
      }
      _ => return Err(e.into()),
    }
  }

  Ok(())
}

pub fn read_settings_json(path: &Path) -> Result<PersistedSettings> {
  let mut s = String::new();
  File::open(path)
    .unwrap()
    .read_to_string(&mut s)
    .unwrap();

  Ok(serde_json::from_str(&s)?)
}

pub fn write_settings_json(path: &Path, content: &PersistedSettings) -> Result<()> {
  let file = OpenOptions::new().write(true).open(path)?;

  Ok(serde_json::to_writer_pretty(file, content)?)
}
