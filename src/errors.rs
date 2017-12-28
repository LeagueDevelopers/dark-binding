#![allow(unused_doc_comment)]

use serde_json;
use websocket;
use reqwest;

error_chain! {
  foreign_links {
    Io(::std::io::Error);
    ParseInt(::std::num::ParseIntError);
    JsonSerde(serde_json::error::Error);
    Request(reqwest::Error);
    UrlParse(reqwest::UrlError);
    WebSocketError(websocket::WebSocketError);
  }

  errors {
    ForceShutdown {
      display("force shutdown")
      description("force shutdown")
    }
  }
}
