mod dj;
mod error;

#[derive(Debug, Deserialize)]
pub struct Balloon {
    pub problem: String,
    pub team: String,
    pub location: Option<String>,
    pub color: String,
    pub total: std::collections::HashMap<String, String>,
    pub awards: String,
    pub balloonid: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct BalloonOutput {
    pub problem: String,
    pub team: String,
    pub location: String,
    pub color: String,
    pub total: String,
    pub awards: String,
}

impl From<Balloon> for BalloonOutput {
    fn from(b: Balloon) -> Self {
        let Balloon {
            problem,
            team,
            location,
            color,
            total,
            awards,
            ..
        } = b;
        let location = location.unwrap_or_else(|| "unknown".to_owned());
        let total = total.into_keys().collect::<Vec<_>>().join(",");
        BalloonOutput {
            problem,
            team,
            location,
            color,
            total,
            awards,
        }
    }
}

#[derive(Deserialize)]
struct DomJudge {
    url: String,
    contest_id: String,
    user: String,
    password: String,
}

#[derive(Deserialize)]
struct Config {
    printer: String,
    format: String,
    encoding: String,
    domjudge: DomJudge,
}

pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::Balloon;
    pub use reqwest::Url;
    pub use serde::{Deserialize, Serialize};
    pub use std::time::Duration;
    pub use tokio::sync::{mpsc, oneshot};
}

use prelude::*;

#[derive(PartialEq, Debug)]
enum Command {
    Pause,
    Resume,
}

#[tokio::main]
async fn main() {
    let config = std::fs::read_to_string("config.toml");
    if let Err(e) = config {
        panic!("cannot load the configuration file: {}", e);
    }
    let config = toml::from_str(&config.unwrap());
    if let Err(e) = config {
        panic!("cannot parse the configuration file: {}", e);
    }
    let config: Config = config.unwrap();

    let (tx, mut rx) = mpsc::channel::<Command>(1);
    let (exit_tx, exit_rx) = oneshot::channel::<()>();
    let f = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&config.printer)
        .await;
    if let Err(e) = f {
        panic!("cannot open the printer: {}", e);
    }
    let mut f = f.unwrap();

    let enc =
        encoding_rs::Encoding::for_label(config.encoding.as_bytes()).unwrap_or(encoding_rs::UTF_8);

    let mut serv = dj::DomJudgeRunner::new(
        Url::parse(&config.domjudge.url).unwrap(),
        &config.domjudge.contest_id,
        &config.domjudge.user,
        &config.domjudge.password,
    )
    .await
    .unwrap();
    tokio::spawn(async move {
        let mut pause = false;
        loop {
            if pause {
                if let Some(msg) = rx.recv().await {
                    if msg == Command::Resume {
                        pause = false;
                        continue;
                    }
                } else {
                    break;
                }
            }
            match rx.try_recv() {
                Ok(Command::Pause) => {
                    pause = true;
                    continue;
                }
                Ok(Command::Resume) => pause = false,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            let b = serv.get_balloon().await;
            if let Err(e) = b {
                eprintln!("error get new balloons: {}", e);
                break;
            }
            let b = b.unwrap();
            if b.is_none() {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                    x = rx.recv() => {
                        match x {
                            None => break,
                            Some(Command::Pause) => pause = true,
                            Some(Command::Resume) => pause = false,
                        };
                        continue;
                    },
                }
                continue;
            }
            let b = b.unwrap();
            let id = b.balloonid;
            let b: BalloonOutput = b.into();
            let tt = text_placeholder::Template::new(&config.format);
            let out = tt.fill_with_struct(&b).unwrap();
            use tokio::io::AsyncWriteExt;
            let (encoded_out, _, _) = enc.encode(&out);
            f.write_all(&encoded_out).await.unwrap();
            serv.done_balloon(id).await.unwrap();
        }
        exit_tx.send(()).unwrap();
    });

    let mut rl = rustyline::Editor::<()>::new();
    let mut paused = false;

    loop {
        let prompt = match paused {
            true => "balloon (paused)  >> ",
            false => "balloon (running) >> ",
        };
        let readline = rl.readline(prompt);
        use rustyline::error::ReadlineError;
        match readline.as_deref() {
            Ok("exit") => break,
            Ok("pause") => {
                if tx.send(Command::Pause).await.is_err() {
                    break;
                }
                paused = true;
            }
            Ok("resume") => {
                if tx.send(Command::Resume).await.is_err() {
                    break;
                }
                paused = false;
            }
            Ok("") => {}
            Ok(_) => println!("unknown command"),
            Err(ReadlineError::Interrupted) => {
                paused = !paused;
                let r = match paused {
                    true => tx.send(Command::Pause).await,
                    false => tx.send(Command::Resume).await,
                };
                if r.is_err() {
                    break;
                }
            }
            Err(ReadlineError::Eof) => println!("use ``exit'' to quit"),
            Err(e) => println!("error: {}", e),
        }
    }

    drop(tx);
    let _ = exit_rx.await;
}
