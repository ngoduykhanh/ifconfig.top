#[macro_use]
extern crate serde_json;
extern crate actix_web;
extern crate actix_files;
extern crate maxminddb;
extern crate regex;
extern crate env_logger;

use std::{env};
use std::net::IpAddr;
use std::str::FromStr;
use std::collections::HashMap;
use std::fmt::format;
use serde_json::ser::State;
use serde::Deserialize;
use regex::Regex;
use actix_web::http::{Method};
use actix_web::{get, post, http, web, dev, App, Error, HttpRequest, HttpResponse, Result, HttpServer, Responder};
use actix_web::web::Query;
use actix_web::middleware::Logger;
use actix_web::middleware::errhandlers::{ErrorHandlerResponse, ErrorHandlers};
use actix_files::{NamedFile, Files};
use env_logger::Env;
use maxminddb::geoip2;
use tera::Tera;

#[derive(Deserialize)]
struct ListQuery {
    cmd: Option<String>,
}

#[derive(Deserialize)]
struct PathInfo {
    name: String,
}

fn is_cli(req: &HttpRequest) -> bool {
    let user_agent = format!("{:?}", req.headers().get("user-agent").unwrap());
    let re = Regex::new(r"(curl|wget|Wget|fetch slibfetch)/.*$").unwrap();
    return re.is_match(&user_agent);
}

fn lookup_ip(req: &HttpRequest) -> String {
    return format!("{}", req.peer_addr().unwrap().ip());
}

fn lookup_cmd(cmd: &str) -> &str {
    let s = match cmd {
        "curl" => "curl",
        "wget" => "wget -qO -",
        "fetch" => "fetch -qo -",
        _ => ""
    };
    return s;
}

fn lookup_country(ip_address: &String) -> String {
    // Convert visitor's ip address to country. The GeoLite2-Country.mmdb
    // can be downloaded from https://dev.maxmind.com/geoip/geoip2/geolite2
    let reader = maxminddb::Reader::open_readfile("GeoLite2-Country.mmdb").unwrap();
    let ip: IpAddr = FromStr::from_str(ip_address).unwrap();

    return match reader.lookup(ip) {
        Ok(db) => {
            let db: geoip2::Country = db;
            format!("{:?}", db)
        }
        Err(error) => {
            println!("Error during looking up ip {:?} the DB: {:?}", ip_address, error);
            String::from("Unknown")
        }
    };
}

fn render_template(tera: web::Data<Tera>, template: &str) -> Result<HttpResponse, Error> {
    let s = tera.render(template, &tera::Context::new()).unwrap();
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

async fn favicon() -> Result<NamedFile> {
    Ok(NamedFile::open("static/favicon.ico")?)
}

fn p404(tera: web::Data<Tera>) -> Result<HttpResponse, Error> {
    render_template(tera, "404.html")
}

fn render_500<B>(mut res: dev::ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    res.response_mut().headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("Error"),
    );
    Ok(ErrorHandlerResponse::Response(res))
}


#[get("/")]
async fn index(req: HttpRequest, query: web::Query::<ListQuery>, tera: web::Data<Tera>) -> impl Responder {
    let ip_address = lookup_ip(&req);

    // If user browses the index page by using command line,
    // return the public ip address instead of whole html page.
    if is_cli(&req) {
        return HttpResponse::Ok().content_type("text/plain").body(ip_address + "\n");
    }

    let country = lookup_country(&ip_address);

    // Get the command from cmd query string
    let mut cmd = String::from("curl");
    match &query.cmd {
        None => {}
        Some(val) => {
            cmd = val.to_string()
        }
    }

    // Create a HashMap from request's header
    // for later using with json.
    let mut headers = HashMap::new();
    for (key, value) in req.headers().iter() {
        let k = format!("{:}", key);
        let mut v = format!("{:?}", value);
        v = v.replace('"', "");
        headers.insert(k, v);
    }

    // Remove Host header
    headers.remove("host");
    // Add country and ip address into above HashMap
    headers.insert(String::from("country"), format!("{:}", country));
    headers.insert(String::from("ip-address"), format!("{:}", ip_address));

    // Create a Context for template variable rendering
    let mut context = tera::Context::new();
    context.insert("cmd", &cmd);
    context.insert("cmd_with_options", lookup_cmd(&cmd));
    context.insert("ip_address", &ip_address);
    context.insert("headers", &headers);
    context.insert("country", &country);

    let rendered = tera.render("index.html", &context).unwrap();
    HttpResponse::Ok().content_type("text/html").body(rendered)
}

#[get("/{name}")]
async fn custom_query(req: HttpRequest, path: web::Path<PathInfo>, tera: web::Data<Tera>) -> impl Responder {
    let name = &path.name;
    match name.as_ref() {
        "country" => {
            let ip_address = lookup_ip(&req);
            let country = lookup_country(&ip_address);
            return Ok(HttpResponse::Ok()
                .content_type("text/plain")
                .body(country));
        }
        "all.json" => {
            let mut headers = HashMap::new();
            for (key, value) in req.headers().iter() {
                let k = format!("{:}", key);
                let mut v = format!("{:?}", value);
                v = v.replace('"', "");
                headers.insert(k, v);
            }

            let ip_address = lookup_ip(&req);
            let country = lookup_country(&ip_address);

            headers.remove("host");
            headers.insert(String::from("country"), format!("{:}", country));
            headers.insert(String::from("ip-address"), format!("{:}", ip_address));
            let result = serde_json::to_string_pretty(&headers).unwrap();
            return Ok(HttpResponse::Ok().content_type("application/json").body(result));
        }
        _ => {
            if req.headers().contains_key(name) {
                let output = format!("{:?}", req.headers().get(name).unwrap());
                return Ok(HttpResponse::Ok().content_type("text/plain").body(output));
            } else {
                if is_cli(&req) {
                    return Ok(HttpResponse::Ok().content_type("text/html").body(""));
                }
                render_template(tera, "404.html")
            }
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let tera = match Tera::new("templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };
        App::new()
            .data(tera)
            .service(Files::new("/static", "./static").show_files_listing())
            .service(index)
            .service(custom_query)
            // .default_service(p404())
    })
        .bind("0.0.0.0:9292")?
        .run()
        .await
}
