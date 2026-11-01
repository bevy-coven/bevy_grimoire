use crate::{Config, Error, Request, Response};
use std::path::{Path, PathBuf};

/// Something that answers a [`Request`]. [`Http`] for a live endpoint, [`Replay`] for a recorded one.
pub trait Transport {
    fn endpoint(&self) -> String;
    fn send(&self, request: &Request) -> Result<Response, Error>;
}

/// A live endpoint over HTTP.
pub struct Http {
    pub config: Config,
}

impl Http {
    pub fn new(config: Config) -> Self {
        Http { config }
    }
}

impl Transport for Http {
    fn endpoint(&self) -> String {
        self.config.endpoint.clone()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        let endpoint = self.config.endpoint.clone();
        let mut body = request.clone();
        if body.model.is_none() {
            body.model = self.config.model.clone();
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(self.config.timeout_secs)))
            // A refusal carries the endpoint's own complaint in its body.
            .http_status_as_error(false)
            .build()
            .into();
        let mut req = agent.post(&endpoint);
        if let Some(key) = &self.config.key {
            req = req.header("Authorization", &format!("Bearer {key}"));
        }
        let mut resp = req
            .send_json(&body)
            .map_err(|e| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() })?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() })?;
        if !(200..300).contains(&status) {
            return Err(Error::Refused { endpoint, status, body: text.chars().take(500).collect() });
        }
        serde_json::from_str(&text).map_err(|e| Error::BadResponse { endpoint, detail: e.to_string() })
    }
}

/// A recorded exchange, answered with no network. It answers only the request it was recorded for:
/// a different request is an error, never the recorded answer.
pub struct Replay {
    request: Request,
    response: Response,
}

impl Replay {
    pub fn new(request: Request, response: Response) -> Self {
        Replay { request, response }
    }

    /// Read `request.json` and `response.json` from a fixture directory.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        fn read<T: serde::de::DeserializeOwned>(p: PathBuf) -> Result<T, Error> {
            let text = std::fs::read_to_string(&p).map_err(|e| Error::Fixture(format!("{}: {e}", p.display())))?;
            serde_json::from_str(&text).map_err(|e| Error::Fixture(format!("{}: {e}", p.display())))
        }
        Ok(Replay { request: read(dir.join("request.json"))?, response: read(dir.join("response.json"))? })
    }
}

impl Transport for Replay {
    fn endpoint(&self) -> String {
        "replay".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        if *request != self.request {
            return Err(Error::Fixture("the request is not the one this fixture was recorded for".into()));
        }
        Ok(self.response.clone())
    }
}

/// Wraps a live transport and writes each exchange to `dir` as `request.json` and `response.json`
/// (the last exchange wins). Used by smoke runs that make a new fixture.
pub struct Recorder<T: Transport> {
    inner: T,
    dir: PathBuf,
}

impl<T: Transport> Recorder<T> {
    pub fn new(inner: T, dir: impl Into<PathBuf>) -> Self {
        Recorder { inner, dir: dir.into() }
    }
}

impl<T: Transport> Transport for Recorder<T> {
    fn endpoint(&self) -> String {
        self.inner.endpoint()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let response = self.inner.send(request)?;
        let write = |name: &str, v: String| {
            std::fs::create_dir_all(&self.dir)
                .and_then(|_| std::fs::write(self.dir.join(name), v))
                .map_err(|e| Error::Fixture(format!("{}: {e}", self.dir.join(name).display())))
        };
        let to_s = |e: serde_json::Error| Error::Fixture(e.to_string());
        let r = write("request.json", serde_json::to_string_pretty(request).map_err(to_s)?)
            .and_then(|_| write("response.json", serde_json::to_string_pretty(&response).map_err(to_s)?));
        r?;
        Ok(response)
    }
}
