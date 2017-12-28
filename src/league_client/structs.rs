use serde_json::Value;

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Error {
  error_code: String,
  http_status: u16,
  message: String
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct LocalSummoner {
  display_name: String,
  #[serde(deserialize_with = "to_str")]
  account_id: String,
  #[serde(deserialize_with = "to_str")]
  pub summoner_id: String,
  summoner_level: u16,
  profile_icon_id: u32
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct RSO {
  pub current_platform_id: String
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TeamMember {
  #[serde(deserialize_with = "to_str")]
  pub summoner_id: String,
  pub champion_id: u32
}

#[derive(Debug, PartialEq)]
pub enum ChampionSelectTimerPhase {
  BanPick,
  Finalization,
  Other
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Deserialize)]
pub struct ChampionSelectTimer {
  pub phase: ChampionSelectTimerPhase
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Deserialize)]
pub struct ChampionSelectSessionUpdate {
  pub my_team: Vec<TeamMember>,
  pub timer: ChampionSelectTimer
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct InnerSettingsFile {
  pub name: String,
  pub sections: Value
}

#[serde(rename_all = "camelCase")]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedSettings {
  pub description: String,
  pub files: Vec<InnerSettingsFile>
}

use serde::{Deserialize, Deserializer, de};

fn to_str<'de, D>(de: D) -> Result<String, D::Error> where D: Deserializer<'de> {
  let deser_result: Value = Deserialize::deserialize(de).unwrap();

  match deser_result {
    Value::String(s) => Ok(s),
    Value::Number(ref n) => Ok(n.to_string()),
    Value::Bool(b) if b == true => Ok("true".to_owned()),
    Value::Bool(b) if b == false => Ok("false".to_owned()),
    _ => Err(de::Error::custom("Unexpected value")),

  }
}

impl<'de> Deserialize<'de> for ChampionSelectTimerPhase {
  fn deserialize<D>(de: D) -> Result<ChampionSelectTimerPhase, D::Error>
    where D: Deserializer<'de> {
    let deser_result: Value = Deserialize::deserialize(de).unwrap();

    match deser_result {
      Value::String(s) => {
        Ok(
          match s.as_str() {
            "BAN_PICK" => ChampionSelectTimerPhase::BanPick,
            "FINALIZATION" => ChampionSelectTimerPhase::Finalization,
            _ => ChampionSelectTimerPhase::Other,
          }
        )
      }
      _ => Err(de::Error::custom("Unexpected value")),
    }
  }
}
