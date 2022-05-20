pub use crate::prelude::*;
use std::collections::VecDeque;

pub struct DomJudgeRunner {
    cl: reqwest::Client,
    cl_iface: reqwest::Client,
    buf: VecDeque<Balloon>,
    balloon_api: Url,
    balloon_jury_iface: Url,
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

        let cl_iface = reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .map_err(Error::HttpError)?;

        let login = url.join("login").unwrap();
        let body = cl_iface
            .get(login.clone())
            .send()
            .await
            .map_err(Error::HttpError)?
            .text()
            .await
            .map_err(Error::HttpError)?;

        use regex::Regex;
        let re = Regex::new(r"name=._csrf_token.*value=.(.*).>").unwrap();
        let cap = re.captures(&body);
        let csrf = match cap.and_then(|x| x.get(1)) {
            Some(csrf) => csrf,
            None => return Err(Error::CsrfError),
        };

        let mut params = std::collections::HashMap::new();
        params.insert("_csrf_token", csrf.as_str());
        params.insert("_username", user.as_ref());
        params.insert("_password", passwd.as_ref());

        cl_iface
            .post(login)
            .form(&params)
            .send()
            .await
            .map_err(Error::HttpError)?;

        let path = format!("api/v4/contests/{}/balloons", cid.as_ref());
        // Use `unwrap` because path can't contain invalid bytes.
        let balloon_api = url.join(&path).unwrap();
        let balloon_jury_iface = url.join("jury/balloons/").unwrap();

        Ok(Self {
            cl,
            cl_iface,
            balloon_api,
            balloon_jury_iface,
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
        let url = self
            .balloon_jury_iface
            .join(&(id.to_string() + "/done"))
            .unwrap();
        self.cl_iface
            .get(url)
            .send()
            .await
            .map_err(Error::HttpError)?;
        Ok(())
    }
}
