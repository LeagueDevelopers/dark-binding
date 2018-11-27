use websocket::OwnedMessage;
use websocket::ws::dataframe::DataFrame;
use serde::de::{Deserialize, Visitor, Deserializer, Error as DeserError};
use std::result::Result as StdResult;
use std::fmt;
use serde_json::{from_slice, from_value, Value};
use regex::Regex;

use league_client::LeagueClient;
use league_client::structs::*;
use errors::*;

lazy_static! {
  static ref PID_REGEX: Regex = Regex::new("--app-pid=(\\d+)\"").unwrap();
}

#[derive(Debug)]
enum EventType {
  Create,
  Update,
  Delete
}

impl<'de> Deserialize<'de> for EventType {
  fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error> where D: Deserializer<'de> {
    struct EventTypeVisitor;

    impl<'de> Visitor<'de> for EventTypeVisitor {
      type Value = EventType;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Create | Update | Delete")
      }

      fn visit_str<E>(self, value: &str) -> StdResult<EventType, E> where E: DeserError {
        match value {
          "Create" => Ok(EventType::Create),
          "Update" => Ok(EventType::Update),
          "Delete" => Ok(EventType::Delete),
          _ => Err(E::custom(format!("unknown EventType value: {}", value))),
        }
      }
    }

    deserializer.deserialize_str(EventTypeVisitor)
  }
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, Deserialize)]
struct WampBody {
  event_type: EventType,
  uri: String,
  data: Value
}

fn base_on_message(msg: OwnedMessage) -> Result<Option<WampBody>> {
  if !msg.is_data() {
    return Ok(None);
  }

  let wamp_message: WampMessage = from_slice(&msg.take_payload())?;

  if wamp_message.1 != "OnJsonApiEvent" {
    return Ok(None);
  }

  Ok(Some(wamp_message.2))
}

#[derive(Debug, Deserialize)]
struct WampMessage(u32, String, WampBody);

pub trait LeagueSocketHandler {
  fn on_message(&mut self, msg: OwnedMessage) {
    let event: WampBody;

    match base_on_message(msg).ok() {
      Some(Some(evt)) => {
        event = evt;
      }
      _ => {
        return;
      }
    }

    match event.uri.as_str() {
      "/lol-champ-select/v1/session" => {
        match event.event_type {
          EventType::Update => {
            self.handle_champ_select_v1_update(event.data);
          }
          _ => {}
        }
      }
      _ => {}
    };
  }

  fn handle_champ_select_v1_update(&mut self, data: Value) -> Result<()>;
}

impl LeagueSocketHandler for LeagueClient {
  fn handle_champ_select_v1_update(&mut self, data: Value) -> Result<()> {
    // debug!("{:?}", data);
    let update: ChampionSelectSessionUpdate = from_value(data)?;

    if update.timer.phase == ChampionSelectTimerPhase::Finalization {
      self
        .local_summoner()
        .map(|s| s.summoner_id)
        .and_then(
          |current_summoner_id| {
            update
              .my_team
              .iter()
              .find(|&m| m.summoner_id == current_summoner_id)
              .map(|member| member.champion_id)
          }
        )
        .map(|champion_id| self.load_champion_config(champion_id));
    }

    Ok(())
  }
}
