#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

use std::collections::HashMap;
use std::env;

use regex::{Captures, Regex};
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION};
use rocket::response::Redirect;
use rocket_contrib::{
    databases::redis::{self, Commands},
    json::Json,
};
use uuid::Uuid;

mod error;

#[database("redis-db")]
struct RedisDB(redis::Connection);

#[derive(Serialize)]
struct StateResp {
    state: String,
}

#[derive(Serialize, Debug)]
struct Message {
    status: i32,
    message: String,
}

#[derive(Deserialize, Debug)]
struct GithubTokenResp {
    access_token: String,
    scope: String,
    token_type: String,
}

#[derive(Deserialize, Debug)]
struct AnalyzeRequest {
    state: String,
}

#[derive(Deserialize, Debug)]
struct RepoInfo {
    id: i32,
    language: Option<String>,
}

#[get("/state")]
fn new_state() -> Json<StateResp> {
    let uuid = Uuid::new_v4();
    let uuid = base64::encode(uuid.as_bytes());

    Json(StateResp { state: uuid })
}

#[get("/cb?<code>&<state>")]
fn oauth_cb(code: String, state: String, conn: RedisDB) -> Result<Redirect, Message> {
    let mut body = HashMap::new();
    body.insert("client_id", env::var("CLIENT_ID").unwrap());
    body.insert("client_secret", env::var("CLIENT_SECRET").unwrap());
    body.insert("code", code);

    let client = reqwest::Client::new();
    let mut resp = client
        .post("https://github.com/login/oauth/access_token")
        .json(&body)
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|_| Message {
            status: 0,
            message: "error when acquire token".to_owned(),
        })?;

    let resp_body: GithubTokenResp = resp.json().map_err(|_| Message {
        status: 0,
        message: "error when acquire token".to_owned(),
    })?;

    let _: () = conn.0.set_ex(state, resp_body.access_token, 3600)?;

    Ok(Redirect::temporary("/"))
}

#[post("/stars", format = "json", data = "<req>")]
fn analyze_stars(
    req: Json<AnalyzeRequest>,
    conn: RedisDB,
) -> Result<Json<HashMap<String, i32>>, Message> {
    let re = Regex::new("page=(\\d+)").unwrap();

    // fetch token by state
    let token: Option<String> = conn.0.get(req.state.clone())?;

    if token.is_none() {
        return Err(Message {
            status: 4,
            message: "unauthorized".to_owned(),
        });
    }

    let token = token.unwrap();

    let client = reqwest::Client::new();

    let mut resp = client
        .get("https://api.github.com/user/starred")
        .header(AUTHORIZATION, format!("token {}", token))
        .send()?;

    let link: Option<&HeaderValue> = resp.headers().get("Link");
    // todo : 处理这两个错误
    let captures: Vec<Captures> = re.captures_iter(link.unwrap().to_str().unwrap()).collect();
    let total_page: i32 = captures[1].get(1).unwrap().as_str().parse().unwrap();

    let mut all_repos: Vec<RepoInfo> = resp.json()?;

    for i in 2..=total_page {
        println!("fetch page {}", i);
        let url = format!("https://api.github.com/user/starred?page={}", i);

        let mut resp = client
            .get(&url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()?;

        let mut repos = resp.json()?;

        all_repos.append(&mut repos);
    }

    let mut analyze = HashMap::new();

    all_repos
        .into_iter()
        .filter(|it| it.language.is_some())
        .for_each(|it| {
            let lang = it.language.unwrap();
            let _value = analyze.get_mut(&lang);

            match analyze.get_mut(&lang) {
                Some(value) => *value += 1,
                None => {
                    analyze.insert(lang, 1);
                }
            }
            ()
        });

    Ok(Json(analyze))
}

fn main() {
    dotenv::dotenv().ok();

    rocket::ignite()
        .attach(RedisDB::fairing())
        .mount("/api", routes![new_state, oauth_cb, analyze_stars])
        .launch();
}