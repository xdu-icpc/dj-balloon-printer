pub use crate::prelude::*;
use std::collections::VecDeque;

pub struct DomJudgeRunner {
    cl: reqwest::Client,
    buf: VecDeque<Balloon>,
    balloon_api: Url,
}

#[derive(Debug, Deserialize)]
struct DomjudgeBalloon {
    #[serde(flatten)]
    b: Balloon,
    done: bool,
}

impl DomJudgeRunner {
    pub async fn new<S0, S1, S2>(url: Url, cid: S0, user: S1, passwd: S2) -> Result<Self>
    where
        S0: AsRef<str>,
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        use reqwest::header;
        let cred = user.as_ref().to_owned() + ":" + passwd.as_ref();
        let auth = "Basic ".to_owned() + &base64::encode(cred.as_bytes());
        // Use `unwrap` because base64 can't contain invalid bytes.
        let mut auth_value = header::HeaderValue::from_str(&auth).unwrap();
        auth_value.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, auth_value);

        let cl = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(Error::HttpError)?;

        let path = format!("api/v4/contests/{}/balloons", cid.as_ref());
        // Use `unwrap` because path can't contain invalid bytes.
        let balloon_api = url.join(&path).unwrap();

        Ok(Self {
            cl,
            balloon_api,
            buf: VecDeque::new(),
        })
    }

    pub async fn get_balloon(&mut self) -> Result<Option<Balloon>> {
        if self.buf.is_empty() {
            let r = self
                .cl
                .get(self.balloon_api.clone())
                .send()
                .await
                .map_err(Error::HttpError)?;
            let r = r
                .json::<Vec<DomjudgeBalloon>>()
                .await
                .map_err(Error::HttpError)?;
            self.buf
                .extend(r.into_iter().filter(|x| !x.done).map(|x| x.b));
        }
        Ok(self.buf.pop_front())
    }

    pub async fn done_balloon(&mut self, id: usize) -> Result<()> {
        // Use `unwrap` because path can't contain invalid bytes.
        let url = self
            .balloon_api
            .join(&id.to_string())
            .unwrap()
            .join("done")
            .unwrap();
        self
            .cl
            .post(url)
            .send()
            .await
            .map_err(Error::HttpError)?;
        Ok(())
    }
}
