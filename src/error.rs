use super::Message;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::response::Responder;
use rocket::Request;
use rocket::Response;

use serde_json::to_string;
use std::io::Cursor;

impl<'a> Responder<'a> for Message {
    fn respond_to(self, _request: &Request) -> Result<Response<'a>, Status> {
        let json = to_string(&self).unwrap();

        Response::build()
            .header(ContentType::JSON)
            .sized_body(Cursor::new(json))
            .ok()
    }
}

impl From<reqwest::Error> for Message {
    fn from(err: reqwest::Error) -> Self {
        println!("{:?}", err);
        Message {
            status: 0,
            message: "error when request".to_owned(),
        }
    }
}

impl From<redis::RedisError> for Message {
    fn from(err: redis::RedisError) -> Self {
        println!("{:?}", err);

        Message {
            status: 0,
            message: "error connect to database".to_owned(),
        }
    }
}
